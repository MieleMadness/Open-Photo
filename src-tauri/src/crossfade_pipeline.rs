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
    struct Cleanup<'a>(&'a Path);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.0);
        }
    }
    // Caller must not specify an existing scratch output; a failed preparation
    // must not remove unrelated pre-existing files.
    if fs::symlink_metadata(request.output).is_ok() {
        return Err("Crossfade scratch output already exists.".into());
    }
    let scratch = request.output;
    let engine = request.engine;
    let expected_audio = request.plan.expected_audio_samples;
    let _cleanup = Cleanup(scratch);
    crossfade_command::execute_with_source_verification(
        request,
        || crossfade_integrity::verify_source_fingerprints(sources, &cancel),
        &cancel,
    )?;
    crossfade_audio_count_check(engine, scratch, expected_audio, &cancel)?;
    if cancel() {
        return Err("Crossfade cancelled before publication.".into());
    }
    let length = fs::metadata(scratch).map_err(|e| e.to_string())?.len();
    crossfade_publish::publish_with_sources(scratch, destination, length, sources, &cancel)?;
    Ok(())
}

fn crossfade_audio_count_check(
    engine: &Path,
    output: &Path,
    expected: u64,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    crate::crossfade_audio_count::verify_presentation_samples(engine, output, expected, cancel)
}
