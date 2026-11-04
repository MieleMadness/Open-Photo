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
    let validate = || {
        crate::crossfade_provenance::verify_ordered_receipts(
            snapshot,
            receipts,
            request.inputs,
            &expected_frames,
            &cancel,
        )
    };
    validate()?;
    let expectations = manifest.expectations();
    render_verify_publish_with_part_guard(request, destination, &expectations, validate, cancel)
}

fn crossfade_audio_count_check(
    engine: &Path,
    output: &Path,
    expected: u64,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    crate::crossfade_audio_count::verify_presentation_samples(engine, output, expected, cancel)
}
