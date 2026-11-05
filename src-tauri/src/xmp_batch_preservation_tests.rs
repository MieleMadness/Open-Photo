//! Native XMP bulk-tag behavior on disposable synthetic photos and sidecars.
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_BATCH_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "openphoto-xmp-batch-{}-{}",
            std::process::id(),
            NEXT_BATCH_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn mixed_batch_merges_managed_xmp_but_keeps_foreign_sidecars_and_sources() {
    let fixture = Fixture::new();
    let managed_photo = fixture.root.join("managed.jpg");
    let foreign_photo = fixture.root.join("foreign.jpg");
    fs::write(&managed_photo, b"original-managed-bytes").unwrap();
    fs::write(&foreign_photo, b"original-foreign-bytes").unwrap();

    let managed_sidecar = fixture.root.join("managed.jpg.xmp");
    let existing = crate::render_openphoto_xmp(&["Existing".into()]);
    let third_party = "<third:keep xmlns:third=\"urn:third-party\">Keep Me</third:keep>";
    let with_foreign_metadata = existing.replace(
        "</rdf:Description>",
        &format!("{third_party}</rdf:Description>"),
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
    let first = crate::bulk_create_tags(args.0.clone(), args.1.clone(), args.2.clone()).unwrap();
    assert_eq!(first.created, 0);
    assert_eq!(first.updated, 1);
    assert_eq!(first.skipped, 1);
    assert!(first.errors.is_empty());

    let written = fs::read_to_string(&managed_sidecar).unwrap();
    assert!(written.contains(third_party));
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
    let fixture = Fixture::new();
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
