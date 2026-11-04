//! Test-only verified crossfade publication boundary.
//! A caller must have decoded and validated the scratch MP4 before entry.
use std::{
    fs::{self, File},
    path::Path,
};

/// Copy an already validated scratch MP4 into a uniquely owned staging file,
/// revalidating originals immediately before atomic no-overwrite publication.
pub(crate) fn publish_verified(
    scratch_mp4: &Path,
    destination: &Path,
    expected_bytes: u64,
    verify_originals: impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    publish_verified_cancellable(
        scratch_mp4,
        destination,
        expected_bytes,
        verify_originals,
        || false,
    )
}

/// Cooperative cancellation is checked during fingerprint verification and
/// each 64 KiB staging copy chunk. StagedOutput removes temporary content on
/// any error; no file is published before every verification completes.
pub(crate) fn publish_verified_cancellable(
    scratch_mp4: &Path,
    destination: &Path,
    expected_bytes: u64,
    verify_originals: impl Fn() -> Result<(), String>,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if cancel() {
        return Err("Crossfade publication cancelled.".into());
    }
    if expected_bytes == 0 || expected_bytes > crate::video_render::MAX_EXPORT_BYTES {
        return Err("Crossfade published byte limit is invalid.".into());
    }
    let metadata = fs::symlink_metadata(scratch_mp4).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Crossfade publication requires a regular verified scratch MP4.".into());
    }
    if scratch_mp4.extension().and_then(|e| e.to_str()) != Some("mp4")
        || destination.extension().and_then(|e| e.to_str()) != Some("mp4")
    {
        return Err("Crossfade publication requires MP4 paths.".into());
    }
    if metadata.len() != expected_bytes {
        return Err("Crossfade scratch MP4 size changed before publication.".into());
    }
    // A same-sized scratch MP4 replacement must not silently pass staging.
    let (initial_len, initial_hash) = crate::video_project::fingerprint(scratch_mp4, &cancel)?;
    if initial_len != expected_bytes {
        return Err("Crossfade verified scratch length changed.".into());
    }
    verify_originals()?;
    let mut stage = crate::safe_output::StagedOutput::new(destination)?;
    let mut source = File::open(scratch_mp4).map_err(|e| e.to_string())?;
    crate::crossfade_staging::copy_verified_bytes(&mut source, stage.file(), expected_bytes, &cancel)?;
    stage.file().sync_all().map_err(|e| e.to_string())?;
    let (staged_len, staged_hash) =
        crate::video_project::fingerprint(stage.temporary_path(), &cancel)?;
    if staged_len != expected_bytes || staged_hash != initial_hash {
        return Err("Crossfade staged MP4 differs from verified scratch bytes.".into());
    }
    verify_originals()?;
    let (final_len, final_hash) = crate::video_project::fingerprint(scratch_mp4, &cancel)?;
    if final_len != expected_bytes || final_hash != initial_hash {
        return Err("Crossfade scratch MP4 changed before final publication.".into());
    }
    if cancel() {
        return Err("Crossfade publication cancelled before commit.".into());
    }
    stage.commit()?;
    Ok(())
}

/// Test-only final publication adapter: verify *previously stored* original
/// fingerprints both before staging and immediately before the atomic commit.
/// The native crossfade executor remains separate until installed acceptance.
pub(crate) fn publish_with_sources(
    scratch_mp4: &Path,
    destination: &Path,
    expected_bytes: u64,
    sources: &[crate::crossfade_integrity::SourceExpectation<'_>],
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if cancel() {
        return Err("Crossfade publication cancelled.".into());
    }
    publish_verified_cancellable(
        scratch_mp4,
        destination,
        expected_bytes,
        || crate::crossfade_integrity::verify_source_fingerprints(sources, &cancel),
        &cancel,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, path::PathBuf};

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn source_bound_publication_rejects_changed_origins_and_cancellation() {
        let dir = std::env::temp_dir().join(format!(
            "openphoto-crossfade-source-publication-{}",
            crate::video_project::id()
        ));
        fs::create_dir(&dir).unwrap();
        let _fixture = Fixture(dir.clone());
        let original = dir.join("original.mov");
        let scratch = dir.join("verified.mp4");
        let destination = dir.join("export.mp4");
        fs::write(&original, b"saved original").unwrap();
        fs::write(&scratch, b"verified sample").unwrap();
        let (bytes, sha256) = crate::video_project::fingerprint(&original, || false).unwrap();
        let expected = [crate::crossfade_integrity::SourceExpectation {
            path: &original,
            bytes,
            sha256: &sha256,
        }];
        assert!(publish_with_sources(&scratch, &destination, 15, &expected, || true).is_err());
        assert!(!destination.exists());
        fs::write(&original, b"other original").unwrap();
        assert!(publish_with_sources(&scratch, &destination, 15, &expected, || false).is_err());
        assert!(!destination.exists());
        fs::write(&original, b"saved original").unwrap();
        publish_with_sources(&scratch, &destination, 15, &expected, || false).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"verified sample");
        assert!(publish_with_sources(&scratch, &destination, 15, &expected, || false).is_err());
    }

    #[test]
    fn only_publishes_after_reverification_and_refuses_collisions() {
        let dir = std::env::temp_dir().join(format!(
            "openphoto-crossfade-publication-{}",
            crate::video_project::id()
        ));
        fs::create_dir(&dir).unwrap();
        let _fixture = Fixture(dir.clone());
        let scratch = dir.join("scratch.mp4");
        let destination = dir.join("published.mp4");
        fs::write(&scratch, b"verified test video").unwrap();
        let count = Cell::new(0_u32);
        publish_verified(&scratch, &destination, 19, || {
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(count.get(), 2);
        assert_eq!(fs::read(&destination).unwrap(), b"verified test video");
        assert!(publish_verified(&scratch, &destination, 19, || Ok(())).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"verified test video");
    }

    #[test]
    fn rejects_same_size_scratch_replacement_during_staging() {
        let dir = std::env::temp_dir().join(format!(
            "openphoto-crossfade-stage-hash-{}",
            crate::video_project::id()
        ));
        fs::create_dir(&dir).unwrap();
        let _fixture = Fixture(dir.clone());
        let scratch = dir.join("scratch.mp4");
        let destination = dir.join("destination.mp4");
        fs::write(&scratch, b"before").unwrap();
        let calls = Cell::new(0_u32);
        let result = publish_verified(&scratch, &destination, 6, || {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                fs::write(&scratch, b"after!").unwrap();
            }
            Ok(())
        });
        assert!(result.is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::write(&scratch, b"before").unwrap();
        publish_verified(&scratch, &destination, 6, || Ok(())).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"before");
    }

    #[test]
    fn cancellation_during_staging_removes_partial_copy() {
        let dir = std::env::temp_dir().join(format!(
            "openphoto-crossfade-staging-cancel-{}",
            crate::video_project::id()
        ));
        fs::create_dir(&dir).unwrap();
        let _fixture = Fixture(dir.clone());
        let scratch = dir.join("scratch.mp4");
        let destination = dir.join("destination.mp4");
        fs::write(&scratch, vec![0x5a; 512 * 1024]).unwrap();
        let polls = Cell::new(0_u32);
        let result = publish_verified_cancellable(
            &scratch,
            &destination,
            512 * 1024,
            || Ok(()),
            || {
                polls.set(polls.get() + 1);
                polls.get() >= 2
            },
        );
        assert!(result.is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn rejects_mutated_originals_and_preserves_racing_destinations() {
        let dir = std::env::temp_dir().join(format!(
            "openphoto-crossfade-racing-{}",
            crate::video_project::id()
        ));
        fs::create_dir(&dir).unwrap();
        let _fixture = Fixture(dir.clone());
        let scratch = dir.join("scratch.mp4");
        let destination = dir.join("published.mp4");
        fs::write(&scratch, b"safe video").unwrap();
        let count = Cell::new(0_u32);
        assert!(publish_verified(&scratch, &destination, 10, || {
            count.set(count.get() + 1);
            if count.get() == 2 {
                fs::write(&destination, b"other app").unwrap();
                return Err("Original source changed".into());
            }
            Ok(())
        })
        .is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"other app");
        let files = fs::read_dir(&dir).unwrap().count();
        assert_eq!(files, 2);
        fs::remove_file(&destination).unwrap();
        assert!(publish_verified(&scratch, &destination, 11, || Ok(())).is_err());
        assert!(!destination.exists());
    }
}
