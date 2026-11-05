//! Regression coverage for the existing managed-XMP replace path.
//! These fixtures use disposable synthetic sidecars only, never personal media.
use crate::replace_managed_sidecar;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    sidecar: PathBuf,
}

impl Fixture {
    fn new(original: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "openphoto-xmp-recovery-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let sidecar = root.join("synthetic.jpg.xmp");
        fs::write(&sidecar, original).unwrap();
        Self { root, sidecar }
    }

    fn temporary(&self) -> PathBuf {
        self.root.join("synthetic.jpg.xmp.openphoto.tmp")
    }

    fn backup(&self) -> PathBuf {
        self.root.join("synthetic.jpg.xmp.openphoto.bak")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn managed_replacement_preserves_new_content_and_cleans_recovery_files() {
    let fixture = Fixture::new("prior-xmp");
    replace_managed_sidecar(&fixture.sidecar, "prior-xmp", "new-xmp").unwrap();
    assert_eq!(fs::read_to_string(&fixture.sidecar).unwrap(), "new-xmp");
    assert!(!fixture.temporary().exists());
    assert!(!fixture.backup().exists());
}

#[test]
fn external_edit_with_matching_length_is_not_overwritten() {
    let fixture = Fixture::new("first-content");
    // Simulates a second editor changing bytes after a previously read preview.
    fs::write(&fixture.sidecar, "other-content").unwrap();
    let result = replace_managed_sidecar(&fixture.sidecar, "first-content", "our-new-tags");
    assert!(result.unwrap_err().contains("changed outside Open-Photo"));
    assert_eq!(
        fs::read_to_string(&fixture.sidecar).unwrap(),
        "other-content"
    );
    assert!(!fixture.temporary().exists());
    assert!(!fixture.backup().exists());
}

#[test]
fn preexisting_backup_blocks_replacement_without_mutating_any_file() {
    let fixture = Fixture::new("prior-xmp");
    fs::write(fixture.backup(), "recovery-bytes").unwrap();
    let result = replace_managed_sidecar(&fixture.sidecar, "prior-xmp", "new-xmp");
    assert!(result.unwrap_err().contains("recovery file exists"));
    assert_eq!(fs::read_to_string(&fixture.sidecar).unwrap(), "prior-xmp");
    assert_eq!(
        fs::read_to_string(fixture.backup()).unwrap(),
        "recovery-bytes"
    );
    assert!(!fixture.temporary().exists());
}

#[test]
fn preexisting_temporary_file_blocks_replacement_without_mutating_any_file() {
    let fixture = Fixture::new("prior-xmp");
    fs::write(fixture.temporary(), "partially-staged").unwrap();
    let result = replace_managed_sidecar(&fixture.sidecar, "prior-xmp", "new-xmp");
    assert!(result.unwrap_err().contains("recovery file exists"));
    assert_eq!(fs::read_to_string(&fixture.sidecar).unwrap(), "prior-xmp");
    assert_eq!(
        fs::read_to_string(fixture.temporary()).unwrap(),
        "partially-staged"
    );
    assert!(!fixture.backup().exists());
}

#[test]
fn malformed_utf8_existing_sidecar_is_never_replaced() {
    let fixture = Fixture::new("prior-xmp");
    fs::write(&fixture.sidecar, [0xff, 0xfe]).unwrap();
    assert!(replace_managed_sidecar(&fixture.sidecar, "prior-xmp", "new-xmp").is_err());
    assert_eq!(fs::read(&fixture.sidecar).unwrap(), vec![0xff, 0xfe]);
}

#[test]
fn batch_skips_nonfile_sidecar_without_blocking_other_photos() {
    let fixture = Fixture::new("old");
    let first = fixture.root.join("first.jpg");
    let second = fixture.root.join("second.jpg");
    fs::write(&first, b"untouched-one").unwrap();
    fs::write(&second, b"untouched-two").unwrap();
    fs::create_dir(fixture.root.join("first.jpg.xmp")).unwrap();

    let result = crate::bulk_create_tags(
        fixture.root.to_string_lossy().into_owned(),
        vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ],
        vec!["tag".into()],
    )
    .unwrap();

    assert_eq!(result.created, 1);
    assert_eq!(result.skipped, 1);
    assert!(result.errors.is_empty());
    assert_eq!(fs::read(&first).unwrap(), b"untouched-one");
    assert_eq!(fs::read(&second).unwrap(), b"untouched-two");
    assert!(fixture.root.join("first.jpg.xmp").is_dir());
    assert!(fixture.root.join("second.jpg.xmp").is_file());
}

#[cfg(unix)]
#[test]
fn batch_does_not_follow_dangling_sidecar_symlink() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new("old");
    let source = fixture.root.join("photo.jpg");
    fs::write(&source, b"untouched").unwrap();
    let dangling_target = fixture.root.join("missing-target.xmp");
    let link = fixture.root.join("photo.jpg.xmp");
    symlink(&dangling_target, &link).unwrap();

    let result = crate::bulk_create_tags(
        fixture.root.to_string_lossy().into_owned(),
        vec![source.to_string_lossy().into_owned()],
        vec!["tag".into()],
    )
    .unwrap();

    assert_eq!(result.created, 0);
    assert_eq!(result.skipped, 1);
    assert!(result.errors.is_empty());
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!dangling_target.exists());
    assert_eq!(fs::read(&source).unwrap(), b"untouched");
}

#[cfg(unix)]
#[test]
fn dangling_temporary_symlink_is_detected_before_backup_creation() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new("original-xmp");
    let target = fixture.root.join("missing-target");
    symlink(&target, fixture.temporary()).unwrap();

    let error = replace_managed_sidecar(&fixture.sidecar, "original-xmp", "new-xmp").unwrap_err();
    assert!(error.contains("recovery file exists"));
    assert_eq!(
        fs::read_to_string(&fixture.sidecar).unwrap(),
        "original-xmp"
    );
    assert!(fs::symlink_metadata(fixture.temporary())
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!target.exists());
    assert!(!fixture.backup().exists());
}

#[cfg(unix)]
#[test]
fn dangling_backup_symlink_is_detected_without_staging() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new("original-xmp");
    let target = fixture.root.join("missing-recovery-target");
    symlink(&target, fixture.backup()).unwrap();

    let error = replace_managed_sidecar(&fixture.sidecar, "original-xmp", "new-xmp").unwrap_err();
    assert!(error.contains("recovery file exists"));
    assert_eq!(
        fs::read_to_string(&fixture.sidecar).unwrap(),
        "original-xmp"
    );
    assert!(fs::symlink_metadata(fixture.backup())
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!target.exists());
    assert!(!fixture.temporary().exists());
}

#[test]
fn staged_and_backed_up_unpublished_write_refuses_ambiguous_retries() {
    let fixture = Fixture::new("original-xmp");
    fs::write(fixture.temporary(), b"partially-staged-new-version").unwrap();
    fs::write(fixture.backup(), b"original-xmp").unwrap();

    for expected in ["original-xmp", "new-preview"] {
        let error = replace_managed_sidecar(&fixture.sidecar, expected, "retry-xmp").unwrap_err();
        assert!(error.contains("recovery file exists"));
        assert_eq!(fs::read(&fixture.sidecar).unwrap(), b"original-xmp");
        assert_eq!(
            fs::read(fixture.temporary()).unwrap(),
            b"partially-staged-new-version"
        );
        assert_eq!(fs::read(fixture.backup()).unwrap(), b"original-xmp");
    }
}

#[test]
fn pending_recovery_with_external_edit_never_chooses_a_winner_automatically() {
    let fixture = Fixture::new("original-xmp");
    fs::write(fixture.temporary(), b"openphoto-pending").unwrap();
    fs::write(fixture.backup(), b"original-xmp").unwrap();
    fs::write(&fixture.sidecar, b"external-editor-update").unwrap();

    for expected in ["original-xmp", "external-editor-update"] {
        let error = replace_managed_sidecar(&fixture.sidecar, expected, "retry-xmp").unwrap_err();
        assert!(error.contains("recovery file exists"));
        assert_eq!(
            fs::read(&fixture.sidecar).unwrap(),
            b"external-editor-update"
        );
        assert_eq!(fs::read(fixture.temporary()).unwrap(), b"openphoto-pending");
        assert_eq!(fs::read(fixture.backup()).unwrap(), b"original-xmp");
    }
}
