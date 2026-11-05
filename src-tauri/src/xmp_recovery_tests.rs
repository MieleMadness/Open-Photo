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
