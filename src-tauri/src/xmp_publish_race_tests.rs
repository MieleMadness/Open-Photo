//! Issue #90 deterministic, intentionally ignored red test for an external edit
//! in the compare-to-publish window. Never use personal media.
use crate::{atomic_replace, replace_managed_sidecar_with_publish};
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_RACE_FIXTURE: AtomicU64 = AtomicU64::new(0);

#[test]
#[ignore = "Issue #90: existing writer can overwrite an external editor after final comparison"]
fn external_edit_between_compare_and_successful_publish_must_not_be_lost() {
    let root = loop {
        let id = NEXT_RACE_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "openphoto-xmp-publish-race-{}-{id}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => break path,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("unable to create synthetic fixture: {error}"),
        }
    };
    let original = root.join("photo.jpg");
    let sidecar = root.join("photo.jpg.xmp");
    fs::write(&original, b"original-source-never-touched").unwrap();
    fs::write(&sidecar, "previewed-sidecar").unwrap();

    let result = replace_managed_sidecar_with_publish(
        &sidecar,
        "previewed-sidecar",
        "openphoto-new-sidecar",
        |staged, destination| {
            // Deterministic equivalent of an independent editor publishing
            // after Open-Photo's last pre-publication content check.
            fs::write(destination, "external-editor-newer-sidecar")?;
            atomic_replace(staged, destination)
        },
    );
    let actual = fs::read_to_string(&sidecar).unwrap();
    let source = fs::read(&original).unwrap();
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(source, b"original-source-never-touched");
    assert!(
        result.is_err() && actual == "external-editor-newer-sidecar",
        "Issue #90: a successful replacement must never discard externally published bytes; result={result:?}, actual={actual:?}"
    );
}
