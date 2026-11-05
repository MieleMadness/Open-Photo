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
fn mixed_batch_merges_managed_xmp_but_keeps_foreign_sidecars_and_sources() {
    let fixture = Fixture::new("unrelated-synthetic-sidecar");
    let managed_photo = fixture.root.join("managed.jpg");
    let foreign_photo = fixture.root.join("foreign.jpg");
    fs::write(&managed_photo, b"original-managed-bytes").unwrap();
    fs::write(&foreign_photo, b"original-foreign-bytes").unwrap();

    let managed_sidecar = fixture.root.join("managed.jpg.xmp");
    let existing = crate::render_openphoto_xmp(&["Existing".into()]);
    let with_foreign_metadata = existing.replace(
        "</rdf:Description>",
        "<third:keep xmlns:third=\"urn:third-party\">Keep Me</third:keep></rdf:Description>",
    );
    assert_ne!(with_foreign_metadata, existing);
    fs::write(&managed_sidecar, &with_foreign_metadata).unwrap();

    let foreign_sidecar = fixture.root.join("foreign.jpg.xmp");
    let foreign_xml = existing.replace("Open-Photo", "Third Party");
    fs::write(&foreign_sidecar, &foreign_xml).unwrap();

    let args = (
        fixture.root.to_string_lossy().into_owned(),
        vec![
            managed_photo.to_string_lossy().into_owned(),
            foreign_photo.to_string_lossy().into_owned(),
        ],
        vec!["New keyword".to_string()],
    );
    let first = crate::bulk_create_tags(args.0.clone(), args.1.clone(), args.2.clone())
        .unwrap();
    assert_eq!(first.created, 0);
    assert_eq!(first.updated, 1);
    assert_eq!(first.skipped, 1);
    assert!(first.errors.is_empty());

    let written = fs::read_to_string(&managed_sidecar).unwrap();
    assert!(written.contains("<third:keep xmlns:third=\"urn:third-party\">Keep Me</third:keep>"));
    assert_eq!(
        crate::openphoto_xmp_tags(&written).unwrap(),
        vec!["Existing", "New keyword"]
    );
    assert_eq!(fs::read_to_string(&foreign_sidecar).unwrap(), foreign_xml);
    assert_eq!(fs::read(&managed_photo).unwrap(), b"original-managed-bytes");
    assert_eq!(fs::read(&foreign_photo).unwrap(), b"original-foreign-bytes");

    let repeated = crate::bulk_create_tags(args.0, args.1, args.2).unwrap();
    assert_eq!(repeated.created, 0);
    assert_eq!(repeated.updated, 0);
    assert_eq!(repeated.skipped, 2);
    assert!(repeated.errors.is_empty());
    assert_eq!(fs::read_to_string(&managed_sidecar).unwrap(), written);
    assert_eq!(fs::read_to_string(&foreign_sidecar).unwrap(), foreign_xml);
}

#[test]
fn malformed_managed_xml_is_not_repaired_or_overwritten_by_bulk_tagging() {
    let fixture = Fixture::new("unrelated-synthetic-sidecar");
    let source = fixture.root.join("damaged.jpg");
    let sidecar = fixture.root.join("damaged.jpg.xmp");
    fs::write(&source, b"original-photo-bytes").unwrap();
    fs::write(&sidecar, b"<xmp>truncated").unwrap();

    let result = crate::bulk_create_tags(
        fixture.root.to_string_lossy().into_owned(),
        vec![source.to_string_lossy().into_owned()],
        vec!["New keyword".to_string()],
    )
    .unwrap();
    assert_eq!(result.created, 0);
    assert_eq!(result.updated, 0);
    assert_eq!(result.skipped, 1);
    assert!(result.errors.is_empty());
    assert_eq!(fs::read(&source).unwrap(), b"original-photo-bytes");
    assert_eq!(fs::read(&sidecar).unwrap(), b"<xmp>truncated");
}
