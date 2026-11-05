//! Synthetic-only XMP writer regressions for stale snapshots and recovery artifacts.
//! These tests document existing fail-closed behavior, not atomic compare-and-swap.
use crate::{replace_managed_sidecar, replace_managed_sidecar_with_publish};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    sidecar: PathBuf,
}

impl Fixture {
    fn new(initial: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "openphoto-xmp-stale-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let sidecar = root.join("sample.jpg.xmp");
        fs::write(&sidecar, initial).unwrap();
        Self { root, sidecar }
    }

    fn temp(&self) -> PathBuf {
        self.root.join("sample.jpg.xmp.openphoto.tmp")
    }

    fn backup(&self) -> PathBuf {
        self.root.join("sample.jpg.xmp.openphoto.bak")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn stale_second_writer_cannot_undo_first_writer_or_reopen_old_snapshot() {
    let fixture = Fixture::new("base-snapshot");
    replace_managed_sidecar(&fixture.sidecar, "base-snapshot", "writer-a").unwrap();

    let result = replace_managed_sidecar(&fixture.sidecar, "base-snapshot", "writer-b");
    assert!(result.unwrap_err().contains("changed outside Open-Photo"));
    assert_eq!(fs::read_to_string(&fixture.sidecar).unwrap(), "writer-a");
    assert!(!fixture.temp().exists());
    assert!(!fixture.backup().exists());

    replace_managed_sidecar(&fixture.sidecar, "writer-a", "writer-b").unwrap();
    assert_eq!(fs::read_to_string(&fixture.sidecar).unwrap(), "writer-b");
}

#[test]
fn reserved_backup_artifact_refuses_save_without_mutation() {
    let fixture = Fixture::new("original");
    fs::write(fixture.backup(), "prior-recovery").unwrap();
    for _ in 0..2 {
        let result = replace_managed_sidecar(&fixture.sidecar, "original", "new");
        assert!(result.unwrap_err().contains("recovery file exists"));
        assert_eq!(fs::read(&fixture.sidecar).unwrap(), b"original");
        assert_eq!(fs::read(fixture.backup()).unwrap(), b"prior-recovery");
        assert!(!fixture.temp().exists());
    }
}

#[cfg(unix)]
#[test]
fn dangling_backup_symlink_is_never_reused_as_recovery_file() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new("original");
    let external = fixture.root.join("dangling-external-target");
    symlink(&external, fixture.backup()).unwrap();

    assert!(replace_managed_sidecar(&fixture.sidecar, "original", "new").is_err());
    assert_eq!(fs::read(&fixture.sidecar).unwrap(), b"original");
    assert!(fs::symlink_metadata(fixture.backup())
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!external.exists());
    assert!(!fixture.temp().exists());
}

#[test]
fn failed_publish_keeps_old_sidecar_and_retains_original_backup() {
    let fixture = Fixture::new("expected-xmp");
    let result = replace_managed_sidecar_with_publish(
        &fixture.sidecar,
        "expected-xmp",
        "new-xmp",
        |staged, destination| {
            assert_eq!(destination, fixture.sidecar.as_path());
            assert_eq!(fs::read(staged).unwrap(), b"new-xmp");
            assert_eq!(fs::read(fixture.backup()).unwrap(), b"expected-xmp");
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected replace failure",
            ))
        },
    );

    assert!(result.unwrap_err().contains("recovery backup retained"));
    assert_eq!(fs::read(&fixture.sidecar).unwrap(), b"expected-xmp");
    assert_eq!(fs::read(fixture.backup()).unwrap(), b"expected-xmp");
    assert!(!fixture.temp().exists());
    assert!(replace_managed_sidecar(&fixture.sidecar, "expected-xmp", "retry").is_err());
}

#[test]
fn external_edit_before_failed_publish_keeps_external_bytes() {
    let fixture = Fixture::new("expected-xmp");
    let result = replace_managed_sidecar_with_publish(
        &fixture.sidecar,
        "expected-xmp",
        "new-xmp",
        |_staged, destination| {
            fs::write(destination, "external-writer-data")?;
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected external publisher conflict",
            ))
        },
    );

    assert!(result.unwrap_err().contains("recovery backup retained"));
    assert_eq!(fs::read(&fixture.sidecar).unwrap(), b"external-writer-data");
    assert_eq!(fs::read(fixture.backup()).unwrap(), b"expected-xmp");
    assert!(!fixture.temp().exists());
}
