//! Read-only inspection of Open-Photo XMP recovery directory entries.
//! Do not follow links or interpret the presence of an artifact as permission
//! to overwrite, delete, or automatically recover its contents.
use std::{
    fs::{self, Metadata},
    io,
    path::Path,
};

/// Conservative directory-entry state for future review UI. Presence does not
/// establish that either file contains a valid, restorable XMP snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryEntryState {
    Clear,
    TemporaryOnly,
    BackupOnly,
    TemporaryAndBackup,
    UnexpectedEntry,
}

pub(crate) struct RecoveryInventory {
    pub(crate) temporary: Option<Metadata>,
    pub(crate) backup: Option<Metadata>,
}

impl RecoveryInventory {
    pub(crate) fn entry_state(&self) -> RecoveryEntryState {
        if [self.temporary.as_ref(), self.backup.as_ref()]
            .into_iter()
            .flatten()
            .any(|metadata| !metadata.file_type().is_file())
        {
            return RecoveryEntryState::UnexpectedEntry;
        }
        match (self.temporary.is_some(), self.backup.is_some()) {
            (false, false) => RecoveryEntryState::Clear,
            (true, false) => RecoveryEntryState::TemporaryOnly,
            (false, true) => RecoveryEntryState::BackupOnly,
            (true, true) => RecoveryEntryState::TemporaryAndBackup,
        }
    }

    pub(crate) fn requires_review(&self) -> bool {
        self.entry_state() != RecoveryEntryState::Clear
    }
}

fn inspect_entry(path: &Path) -> Result<Option<Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "Unable to inspect Open-Photo XMP recovery path {}: {error}",
            path.display()
        )),
    }
}

pub(crate) fn inspect_recovery_pair(
    temporary: &Path,
    backup: &Path,
) -> Result<RecoveryInventory, String> {
    Ok(RecoveryInventory {
        temporary: inspect_entry(temporary)?,
        backup: inspect_entry(backup)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new() -> Self {
            loop {
                let path = std::env::temp_dir().join(format!(
                    "openphoto-xmp-recovery-inspect-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Could not create test fixture: {error}"),
                }
            }
        }

        fn temp(&self) -> std::path::PathBuf {
            self.0.join("photo.jpg.xmp.openphoto.tmp")
        }

        fn backup(&self) -> std::path::PathBuf {
            self.0.join("photo.jpg.xmp.openphoto.bak")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn missing_entries_are_not_recovery_artifacts() {
        let fixture = Fixture::new();
        let result = inspect_recovery_pair(&fixture.temp(), &fixture.backup()).unwrap();
        assert_eq!(result.entry_state(), RecoveryEntryState::Clear);
        assert!(!result.requires_review());
        assert!(result.temporary.is_none());
        assert!(result.backup.is_none());
    }

    #[test]
    fn backup_and_staged_versions_are_inspected_without_mutation() {
        let fixture = Fixture::new();
        fs::write(fixture.temp(), b"new-version").unwrap();
        fs::write(fixture.backup(), b"old-version").unwrap();

        let result = inspect_recovery_pair(&fixture.temp(), &fixture.backup()).unwrap();
        assert_eq!(result.entry_state(), RecoveryEntryState::TemporaryAndBackup);
        assert!(result.requires_review());
        assert_eq!(result.temporary.unwrap().len(), 11);
        assert_eq!(result.backup.unwrap().len(), 11);
        assert_eq!(fs::read(fixture.temp()).unwrap(), b"new-version");
        assert_eq!(fs::read(fixture.backup()).unwrap(), b"old-version");
    }

    #[test]
    fn directory_entries_are_not_mistaken_for_absent_files() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.backup()).unwrap();
        let result = inspect_recovery_pair(&fixture.temp(), &fixture.backup()).unwrap();
        assert_eq!(result.entry_state(), RecoveryEntryState::UnexpectedEntry);
        assert!(result.requires_review());
        assert!(result.backup.unwrap().file_type().is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn dangling_links_are_inspected_without_following_them() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        let absent = fixture.0.join("not-created");
        symlink(&absent, fixture.temp()).unwrap();
        let result = inspect_recovery_pair(&fixture.temp(), &fixture.backup()).unwrap();
        assert_eq!(result.entry_state(), RecoveryEntryState::UnexpectedEntry);
        assert!(result.requires_review());
        assert!(result.temporary.unwrap().file_type().is_symlink());
        assert!(!absent.exists());
        assert!(result.backup.is_none());
    }
    #[test]
    fn temporary_only_is_reported_without_guessing_recoverability() {
        let fixture = Fixture::new();
        fs::write(fixture.temp(), b"pending").unwrap();
        let state = inspect_recovery_pair(&fixture.temp(), &fixture.backup()).unwrap();
        assert_eq!(state.entry_state(), RecoveryEntryState::TemporaryOnly);
        assert!(state.requires_review());
        assert_eq!(fs::read(fixture.temp()).unwrap(), b"pending");
    }

    #[test]
    fn backup_only_is_reported_without_automatic_restore() {
        let fixture = Fixture::new();
        fs::write(fixture.backup(), b"prior").unwrap();
        let state = inspect_recovery_pair(&fixture.temp(), &fixture.backup()).unwrap();
        assert_eq!(state.entry_state(), RecoveryEntryState::BackupOnly);
        assert!(state.requires_review());
        assert_eq!(fs::read(fixture.backup()).unwrap(), b"prior");
    }

    #[test]
    fn combined_artifacts_are_ambiguous_even_with_identical_bytes() {
        let fixture = Fixture::new();
        fs::write(fixture.temp(), b"same").unwrap();
        fs::write(fixture.backup(), b"same").unwrap();
        let state = inspect_recovery_pair(&fixture.temp(), &fixture.backup()).unwrap();
        assert_eq!(state.entry_state(), RecoveryEntryState::TemporaryAndBackup);
        assert!(state.requires_review());
    }

}
