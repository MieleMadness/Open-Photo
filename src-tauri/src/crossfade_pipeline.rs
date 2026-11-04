//! Test-only crossfade verification chain. Not connected to production exports.
//! Normalized inputs, original fingerprint expectations and staging policy are
//! required before ever permitting a persisted crossfade project to use it.
use crate::{
    crossfade_command::{self, CrossfadeExecution},
    crossfade_integrity::{self, SourceExpectation},
    crossfade_publish,
};
use std::{fs, path::Path};

/// Render and fully decode a scratch MP4, verify AAC *presentation* samples,
/// then revalidate original media before and during safe no-overwrite publication.
/// A temporary output is deleted even after success; only a verified destination
/// survives. The caller must own the scratch directory and its normalized parts.
pub(crate) fn render_verify_publish(
    request: CrossfadeExecution<'_>,
    destination: &Path,
    sources: &[SourceExpectation<'_>],
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    render_verify_publish_with_part_guard(request, destination, sources, || Ok(()), cancel)
}

/// A normalized-part verifier is checked before FFmpeg, after FFmpeg and again
/// before output staging. The same callback participates in the FFmpeg source
/// verification boundary, closing the render-time replacement window.
fn render_verify_publish_with_part_guard(
    request: CrossfadeExecution<'_>,
    destination: &Path,
    sources: &[SourceExpectation<'_>],
    verify_parts: impl Fn() -> Result<(), String>,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    struct Cleanup<'a>(&'a Path);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.0);
        }
    }
    // Caller must not specify an existing scratch output; a failed preparation
    // must not remove unrelated pre-existing files.
    match fs::symlink_metadata(request.output) {
        Ok(_) => return Err("Crossfade scratch output already exists.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Cannot inspect crossfade scratch output: {error}")),
    }
    let scratch = request.output;
    let engine = request.engine;
    let expected_audio = request.plan.expected_audio_samples;
    let _cleanup = Cleanup(scratch);
    crossfade_command::execute_with_source_verification(
        request,
        || {
            crossfade_integrity::verify_source_fingerprints(sources, &cancel)?;
            verify_parts()
        },
        &cancel,
    )?;
    crossfade_audio_count_check(engine, scratch, expected_audio, &cancel)?;
    verify_parts()?;
    if cancel() {
        return Err("Crossfade cancelled before publication.".into());
    }
    let length = fs::metadata(scratch).map_err(|e| e.to_string())?.len();
    verify_staging_space(destination, length)?;
    crossfade_publish::publish_with_sources(scratch, destination, length, sources, &cancel)?;
    Ok(())
}

/// Experimental saved-project entry boundary. It derives original source
/// identities from the persisted document and trusted relink roots; caller
/// supplied normalized MKVs remain private scratch artifacts that must pass
/// the strict independent input contract in `execute_in_scratch`.
pub(crate) fn render_verify_publish_snapshot(
    request: CrossfadeExecution<'_>,
    destination: &Path,
    snapshot: &crate::video_sequence_render::Snapshot,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    let manifest = crate::crossfade_snapshot::SourceManifest::from_snapshot(snapshot, &cancel)?;
    if request.inputs.len() != snapshot.project.clips.len() {
        return Err("Crossfade normalized parts do not match saved timeline clip count.".into());
    }
    let expectations = manifest.expectations();
    render_verify_publish(request, destination, &expectations, cancel)
}

/// Test-only pathway that binds normalized video bytes to the exact saved
/// clip order throughout rendering. Receipts must be minted by the trusted
/// normalizer; checking caller-created receipts alone does not prove origin.
pub(crate) fn render_verify_publish_snapshot_receipts(
    request: CrossfadeExecution<'_>,
    destination: &Path,
    snapshot: &crate::video_sequence_render::Snapshot,
    receipts: &[crate::crossfade_provenance::NormalizedReceipt],
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    let manifest = crate::crossfade_snapshot::SourceManifest::from_snapshot(snapshot, &cancel)?;
    let expected_frames = crate::video_sequence_render::frame_allocation(&snapshot.project)?;
    if request.fps != snapshot.project.frame_rate
        || request.plan.normalized_clip_frames != expected_frames
    {
        return Err("Crossfade plan differs from the saved project frame clock.".into());
    }
    let scratch = request
        .output
        .parent()
        .ok_or("Crossfade scratch directory is missing.")?;
    crate::crossfade_scratch::validate_scratch_paths(scratch, request.inputs, request.output)?;
    let inputs = request.inputs;
    let validate = || {
        crate::crossfade_provenance::verify_ordered_receipts(
            snapshot,
            receipts,
            inputs,
            &expected_frames,
            &cancel,
        )
    };
    validate()?;
    verify_crossfade_disk_budget(scratch, destination, request.max_bytes)?;
    let expectations = manifest.expectations();
    // Pin the same bytes verified by the saved source manifest and the
    // normalizer's ordered receipts. These handles outlive FFmpeg, AAC decode
    // validation, and atomic staged publication.
    #[cfg(windows)]
    let _pinned_originals = {
        let expected: Vec<_> = expectations
            .iter()
            .map(|source| (source.path, source.bytes, source.sha256))
            .collect();
        crate::crossfade_windows_guard::PinnedMedia::acquire(&expected, &cancel)?
    };
    #[cfg(windows)]
    let _pinned_parts = {
        let expected = receipts
            .iter()
            .map(|receipt| {
                let bytes = fs::metadata(&receipt.part)
                    .map_err(|e| e.to_string())?
                    .len();
                Ok((receipt.part.as_path(), bytes, receipt.part_sha256.as_str()))
            })
            .collect::<Result<Vec<_>, String>>()?;
        crate::crossfade_windows_guard::PinnedMedia::acquire(&expected, &cancel)?
    };
    render_verify_publish_with_part_guard(request, destination, &expectations, validate, &cancel)
}

/// Verify that enough free space remains for a bounded scratch MP4 and
/// an independently staged destination MP4 before starting the encoder.
/// On the same Windows volume both allocations may coexist, so require their
/// *combined* budget instead of two insufficient independent checks.
/// Check real free destination space *again* immediately before staging the
/// measured scratch MP4: normalization and encoding can consume the original
/// preflight reserve while this export is running.
fn verify_staging_space(destination: &Path, bytes: u64) -> Result<(), String> {
    let required = bytes
        .checked_add(16 * 1024 * 1024)
        .ok_or("Crossfade staged output free-space requirement overflow.")?;
    let parent = destination
        .parent()
        .ok_or("Crossfade publication destination has no parent folder.")?;
    let remaining = crate::video_export::available_space(parent)?
        .ok_or("Crossfade destination free space could not be verified.")?;
    if remaining < required {
        return Err("Not enough destination disk space to stage the verified crossfade.".into());
    }
    Ok(())
}

fn verify_crossfade_disk_budget(
    scratch: &Path,
    destination: &Path,
    export_limit: u64,
) -> Result<(), String> {
    const RESERVE: u64 = 16 * 1024 * 1024;
    if export_limit == 0 || export_limit > crate::video_render::MAX_EXPORT_BYTES {
        return Err("Crossfade export byte budget is invalid.".into());
    }
    let target_parent = destination
        .parent()
        .ok_or("Crossfade destination folder is unavailable.")?;
    let scratch_free = crate::video_export::available_space(scratch)?;
    let destination_free = crate::video_export::available_space(target_parent)?;
    let single = export_limit
        .checked_add(RESERVE)
        .ok_or("Crossfade free-space budget overflow.")?;
    if scratch_free.is_some_and(|available| available < single)
        || destination_free.is_some_and(|available| available < single)
    {
        return Err("Insufficient free disk space for crossfade rendering.".into());
    }
    #[cfg(windows)]
    {
        let scratch_canonical = fs::canonicalize(scratch).map_err(|e| e.to_string())?;
        let destination_canonical = fs::canonicalize(target_parent).map_err(|e| e.to_string())?;
        if scratch_canonical.components().next() == destination_canonical.components().next() {
            let combined = export_limit
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(RESERVE))
                .ok_or("Crossfade combined free-space budget overflow.")?;
            if scratch_free.is_some_and(|available| available < combined) {
                return Err(
                    "Insufficient shared-volume space for crossfade scratch and stage.".into(),
                );
            }
        }
    }
    Ok(())
}

fn crossfade_audio_count_check(
    engine: &Path,
    output: &Path,
    expected: u64,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    crate::crossfade_audio_stream::verify_streaming(engine, output, expected, cancel)
}
