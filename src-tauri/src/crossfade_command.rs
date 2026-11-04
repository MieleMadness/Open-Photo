//! Experimental, test-only FFmpeg invocation planner for normalized crossfade parts.
//! The production hard-cut/dip renderer and persisted schema remain unchanged.
use crate::crossfade_preflight::PreparedCrossfade;
use std::{
    collections::HashSet,
    ffi::OsString,
    path::{Path, PathBuf},
};

pub(crate) fn command_args(
    plan: &PreparedCrossfade,
    inputs: &[PathBuf],
    output: &Path,
    fps: u32,
    max_bytes: u64,
) -> Result<Vec<OsString>, String> {
    if !matches!(fps, 30 | 60)
        || inputs.len() < 2
        || inputs.len() != plan.normalized_clip_frames.len()
        || inputs.len() > 200
    {
        return Err("Crossfade command needs 2–200 normalized inputs and valid FPS.".into());
    }
    if max_bytes == 0 || max_bytes > crate::video_render::MAX_EXPORT_BYTES {
        return Err("Crossfade output byte limit is invalid.".into());
    }
    if plan.graph.expected_frames == 0
        || plan.graph.expected_frames > u64::from(fps) * 7200
        || plan.expected_audio_samples
            != plan
                .graph
                .expected_frames
                .checked_mul(48_000)
                .ok_or("Crossfade audio sample count overflow.")?
                / u64::from(fps)
    {
        return Err("Crossfade graph frame/audio clock is invalid.".into());
    }
    if plan.graph.filter_complex.is_empty()
        || plan.graph.filter_complex.len() > 256 * 1024
        || plan.graph.video_output.is_empty()
        || plan.graph.audio_output.is_empty()
    {
        return Err("Crossfade filter graph is invalid.".into());
    }
    if output.extension().and_then(|e| e.to_str()) != Some("mp4") || !output.is_absolute() {
        return Err("Crossfade output must be an absolute MP4 path.".into());
    }
    let mut unique = HashSet::new();
    for input in inputs {
        if !input.is_absolute()
            || input.extension().and_then(|e| e.to_str()) != Some("mkv")
            || input == output
            || !unique.insert(input)
        {
            return Err("Crossfade inputs must be distinct absolute normalized MKV paths.".into());
        }
    }
    // Caller must additionally enforce scratch-directory ownership, source
    // verification, exact decoded media counts and output publication policy.
    let mut args: Vec<OsString> = [
        "-nostdin",
        "-v",
        "error",
        "-xerror",
        "-n",
        "-max_alloc",
        "268435456",
        "-threads",
        "2",
        "-filter_complex_threads",
        "1",
        "-protocol_whitelist",
        "file,pipe",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    for input in inputs {
        args.extend([
            "-format_whitelist".into(),
            "matroska".into(),
            "-i".into(),
            input.as_os_str().to_owned(),
        ]);
    }
    args.extend([
        "-filter_complex".into(),
        plan.graph.filter_complex.clone().into(),
        "-map".into(),
        format!("[{}]", plan.graph.video_output).into(),
        "-map".into(),
        format!("[{}]", plan.graph.audio_output).into(),
        "-frames:v".into(),
        plan.graph.expected_frames.to_string().into(),
        "-t".into(),
        format!("{:.9}", plan.graph.expected_frames as f64 / f64::from(fps)).into(),
        "-c:v".into(),
        "libopenh264".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-ar".into(),
        "48000".into(),
        "-ac".into(),
        "2".into(),
        "-map_metadata".into(),
        "-1".into(),
        "-map_chapters".into(),
        "-1".into(),
        "-fs".into(),
        max_bytes.to_string().into(),
        "-f".into(),
        "mp4".into(),
        output.as_os_str().to_owned(),
    ]);
    Ok(args)
}

/// Experimental test-only executor. Never use this with untrusted or user-owned
/// paths: the caller must own the scratch directory and verified normalized parts.
pub(crate) fn execute_in_scratch(
    engine: &Path,
    plan: &PreparedCrossfade,
    inputs: &[PathBuf],
    output: &Path,
    fps: u32,
    max_bytes: u64,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    crate::crossfade_scratch::validate_scratch_paths(
        output
            .parent()
            .ok_or("Crossfade output folder is missing.")?,
        inputs,
        output,
    )?;
    // Probe each normalized part before starting the expensive graph. Missing
    // streams, mismatched canvas, frame-rate drift and invalid audio fail closed.
    let mut normalized = Vec::with_capacity(inputs.len());
    for part in inputs {
        if cancel() {
            return Err("Crossfade cancelled while verifying normalized inputs.".into());
        }
        let mut info = crate::video_probe::probe(engine, part, &cancel)?;
        // Matroska commonly omits nb_frames. Count decoded video frames
        // explicitly instead of trusting an estimated duration or frame rate.
        let ffprobe = crate::video_engine::tool(engine, "ffprobe")?;
        let count_args: [&std::ffi::OsStr; 14] = [
            "-v".as_ref(),
            "error".as_ref(),
            "-max_alloc".as_ref(),
            "268435456".as_ref(),
            "-protocol_whitelist".as_ref(),
            "file,pipe".as_ref(),
            "-format_whitelist".as_ref(),
            "matroska".as_ref(),
            "-count_frames".as_ref(),
            "-select_streams".as_ref(),
            "v:0".as_ref(),
            "-show_entries".as_ref(),
            "stream=nb_read_frames".as_ref(),
            "-of".as_ref(),
        ];
        let mut args = count_args.to_vec();
        args.push("default=noprint_wrappers=1:nokey=1".as_ref());
        args.push(part.as_os_str());
        let measured = crate::video_engine::run_until(
            &ffprobe,
            &args,
            std::time::Duration::from_secs(120),
            &cancel,
        )?;
        let measured_frames = String::from_utf8(measured)
            .map_err(|_| "Crossfade decoded frame count is not valid UTF-8.")?
            .trim()
            .parse::<u64>()
            .map_err(|_| "Crossfade decoded frame count is unavailable.")?;
        if info
            .frame_count
            .is_some_and(|reported| reported != measured_frames)
        {
            return Err("Crossfade declared and decoded frame counts disagree.".into());
        }
        info.frame_count = Some(measured_frames);
        normalized.push(info);
    }
    crate::crossfade_inputs::validate_normalized_parts(
        &normalized,
        &plan.normalized_clip_frames,
        fps,
    )?;
    let args = command_args(plan, inputs, output, fps, max_bytes)?;
    struct Cleanup<'a> {
        output: &'a Path,
        verified: bool,
    }
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            if !self.verified {
                let _ = std::fs::remove_file(self.output);
            }
        }
    }
    let mut cleanup = Cleanup {
        output,
        verified: false,
    };
    if cancel() {
        return Err("Crossfade cancelled before execution.".into());
    }
    let ffmpeg = crate::video_engine::tool(engine, "ffmpeg")?;
    crate::video_engine::run_until(
        &ffmpeg,
        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
        std::time::Duration::from_secs(120),
        &cancel,
    )?;
    if cancel() {
        return Err("Crossfade cancelled after execution.".into());
    }
    let result = crate::video_probe::probe(engine, output, &cancel)?;
    if cancel() {
        return Err("Crossfade cancelled during output validation.".into());
    }
    let bytes = std::fs::metadata(output).map_err(|e| e.to_string())?.len();
    if bytes == 0 || bytes > max_bytes {
        return Err("Crossfade generated an empty or oversized output.".into());
    }
    let duration = plan.graph.expected_frames as f64 / f64::from(fps);
    if result.frame_count != Some(plan.graph.expected_frames)
        || (result.duration_seconds - duration).abs() > 0.05
        || result.audio.len() != 1
        || result.audio[0].sample_rate.as_deref() != Some("48000")
        || (result.audio[0]
            .duration_seconds
            .unwrap_or(result.duration_seconds)
            - duration)
            .abs()
            > 0.05
    {
        return Err("Crossfade output frame/audio verification failed.".into());
    }
    // Container/probe metadata alone cannot catch corrupted decoded frames.
    // Reuse the existing bounded, cancellable full-output decode check.
    crate::video_render::decode_check(engine, output, &cancel, |_| {})?;
    if cancel() {
        return Err("Crossfade cancelled during decoded-output verification.".into());
    }
    cleanup.verified = true;
    Ok(())
}

/// Test-only source verification boundary. The caller must provide an
/// independently stored fingerprint verifier for *original* timeline files.
/// Keep the rendered output private if an original changes mid-export.
pub(crate) struct CrossfadeExecution<'a> {
    pub engine: &'a Path,
    pub plan: &'a PreparedCrossfade,
    pub inputs: &'a [PathBuf],
    pub output: &'a Path,
    pub fps: u32,
    pub max_bytes: u64,
}

pub(crate) fn execute_with_source_verification(
    request: CrossfadeExecution<'_>,
    verify_sources: impl Fn() -> Result<(), String>,
    cancel: impl Fn() -> bool,
) -> Result<(), String> {
    if cancel() {
        return Err("Crossfade source verification cancelled.".into());
    }
    verify_sources()?;
    if cancel() {
        return Err("Crossfade source verification cancelled.".into());
    }
    execute_in_scratch(
        request.engine,
        request.plan,
        request.inputs,
        request.output,
        request.fps,
        request.max_bytes,
        &cancel,
    )?;
    // Never expose an MP4 whose original source changed while FFmpeg worked.
    if let Err(error) = verify_sources() {
        let _ = std::fs::remove_file(request.output);
        return Err(error);
    }
    if cancel() {
        let _ = std::fs::remove_file(request.output);
        return Err("Crossfade cancelled after source verification.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crossfade_clock::{Boundary, ClipClock},
        crossfade_preflight::plan_allocated,
    };

    fn fixture() -> (PreparedCrossfade, Vec<PathBuf>, PathBuf) {
        let clips = [
            ClipClock {
                id: "first",
                frames: 90,
            },
            ClipClock {
                id: "second",
                frames: 120,
            },
        ];
        let edges = [Boundary {
            from_clip_id: "first",
            to_clip_id: "second",
            frames: 15,
        }];
        let plan = plan_allocated(&clips, &edges, &[true, false], 30).unwrap();
        let dir = std::env::temp_dir().join("openphoto-crossfade-plan-test");
        (
            plan,
            vec![dir.join("part-0.mkv"), dir.join("part-1.mkv")],
            dir.join("result.mp4"),
        )
    }

    #[test]
    fn builds_bounded_input_only_graph_command() {
        let (plan, parts, output) = fixture();
        let args = command_args(&plan, &parts, &output, 30, 64 * 1024 * 1024).unwrap();
        let values: Vec<String> = args
            .iter()
            .map(|v| v.to_string_lossy().into_owned())
            .collect();
        assert_eq!(values.iter().filter(|v| *v == "-i").count(), 2);
        assert!(values.contains(&"-filter_complex".to_owned()));
        assert!(values.contains(&"-frames:v".to_owned()));
        assert!(values.contains(&"195".to_owned()));
        assert!(values.contains(&"6.500000000".to_owned()));
        assert!(values.contains(&"file,pipe".to_owned()));
        assert!(values.contains(&"-n".to_owned()));
        assert!(!values.contains(&"-y".to_owned()));
        assert_eq!(values.last(), Some(&output.to_string_lossy().into_owned()));
    }

    #[test]
    fn rejects_unbounded_or_untrusted_input_shapes() {
        let (mut plan, parts, output) = fixture();
        assert!(command_args(&plan, &parts[..1], &output, 30, 64 * 1024 * 1024).is_err());
        assert!(command_args(&plan, &parts, &output, 24, 64 * 1024 * 1024).is_err());
        assert!(command_args(&plan, &parts, &output, 30, 0).is_err());
        assert!(command_args(&plan, &parts, &output, 30, u64::MAX).is_err());
        assert!(command_args(
            &plan,
            &[parts[0].clone(), parts[0].clone()],
            &output,
            30,
            1024
        )
        .is_err());
        assert!(command_args(
            &plan,
            &[PathBuf::from("relative.mkv"), parts[1].clone()],
            &output,
            30,
            1024
        )
        .is_err());
        assert!(command_args(&plan, &parts, &output.with_extension("mkv"), 30, 1024).is_err());
        plan.expected_audio_samples += 1;
        assert!(command_args(&plan, &parts, &output, 30, 1024).is_err());
    }
}

#[cfg(all(test, windows))]
mod bundled_command_tests {
    use super::*;
    use crate::{
        crossfade_clock::{Boundary, ClipClock},
        crossfade_preflight::plan_allocated,
        video_engine, video_probe, video_project,
    };
    use std::{fs, time::Duration};

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn executes_bounded_command_and_verifies_audio_video_output() {
        let engine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine");
        let ffmpeg = video_engine::tool(&engine, "ffmpeg").unwrap();
        let dir = std::env::temp_dir().join(format!("crossfade-command-{}", video_project::id()));
        fs::create_dir(&dir).unwrap();
        let dir = fs::canonicalize(&dir).unwrap();
        let _fixture = Fixture(dir.clone());

        let clips = [
            ClipClock {
                id: "red",
                frames: 60,
            },
            ClipClock {
                id: "blue",
                frames: 60,
            },
        ];
        let edges = [Boundary {
            from_clip_id: "red",
            to_clip_id: "blue",
            frames: 15,
        }];
        let preflight = plan_allocated(&clips, &edges, &[true, false], 30).unwrap();
        let mut parts = Vec::new();
        for (index, color) in ["red", "blue"].iter().enumerate() {
            let part = dir.join(format!("part-{index}.mkv"));
            let args: Vec<OsString> = vec![
                "-nostdin".into(),
                "-v".into(),
                "error".into(),
                "-f".into(),
                "lavfi".into(),
                "-i".into(),
                format!("color=c={color}:s=96x64:r=30:d=2").into(),
                "-f".into(),
                "lavfi".into(),
                "-i".into(),
                "anullsrc=r=48000:cl=stereo".into(),
                "-t".into(),
                "2".into(),
                "-frames:v".into(),
                "60".into(),
                "-c:v".into(),
                "libopenh264".into(),
                "-b:v".into(),
                "150000".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
                "-c:a".into(),
                "pcm_f32le".into(),
                part.clone().into(),
            ];
            video_engine::run_until(
                &ffmpeg,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                Duration::from_secs(90),
                || false,
            )
            .unwrap();
            parts.push(part);
        }

        let output = dir.join("result.mp4");
        let args = command_args(&preflight, &parts, &output, 30, 64 * 1024 * 1024).unwrap();
        execute_in_scratch(
            &engine,
            &preflight,
            &parts,
            &output,
            30,
            64 * 1024 * 1024,
            || false,
        )
        .unwrap();

        let result = video_probe::probe(&engine, &output, || false).unwrap();
        assert!(fs::metadata(&output).unwrap().len() <= 64 * 1024 * 1024);
        assert_eq!(result.frame_count, Some(105));
        assert_eq!((result.width, result.height), (96, 64));
        assert!((result.duration_seconds - 3.5).abs() < 0.05);
        assert_eq!(result.audio.len(), 1);
        assert_eq!(result.audio[0].sample_rate.as_deref(), Some("48000"));
        let audio_seconds = result.audio[0]
            .duration_seconds
            .unwrap_or(result.duration_seconds);
        assert!((audio_seconds - 3.5).abs() < 0.05);

        let cancelled_output = dir.join("cancelled.mp4");
        assert!(execute_in_scratch(
            &engine,
            &preflight,
            &parts,
            &cancelled_output,
            30,
            64 * 1024 * 1024,
            || true,
        )
        .is_err());
        assert!(!cancelled_output.exists());

        // Cancel after preflight but during the process-runner lifecycle.
        let cancellation_checks = std::sync::atomic::AtomicUsize::new(0);
        let interrupted_output = dir.join("interrupted.mp4");
        assert!(execute_in_scratch(
            &engine,
            &preflight,
            &parts,
            &interrupted_output,
            30,
            64 * 1024 * 1024,
            || cancellation_checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0,
        )
        .is_err());
        assert!(cancellation_checks.load(std::sync::atomic::Ordering::SeqCst) >= 2);
        assert!(!interrupted_output.exists());

        // A playable MKV with no normalized PCM audio must be rejected before
        // the multi-input graph, even if its video frame count is correct.
        let missing_pcm = dir.join("no-pcm.mkv");
        let missing_pcm_args: Vec<OsString> = vec![
            "-nostdin".into(),
            "-v".into(),
            "error".into(),
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            "color=c=green:s=96x64:r=30:d=2".into(),
            "-t".into(),
            "2".into(),
            "-frames:v".into(),
            "60".into(),
            "-c:v".into(),
            "libopenh264".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-an".into(),
            missing_pcm.clone().into(),
        ];
        video_engine::run_until(
            &ffmpeg,
            &missing_pcm_args
                .iter()
                .map(OsString::as_os_str)
                .collect::<Vec<_>>(),
            Duration::from_secs(90),
            || false,
        )
        .unwrap();
        let missing_pcm_output = dir.join("no-pcm-output.mp4");
        assert!(execute_in_scratch(
            &engine,
            &preflight,
            &[parts[0].clone(), missing_pcm],
            &missing_pcm_output,
            30,
            64 * 1024 * 1024,
            || false,
        )
        .is_err());
        assert!(!missing_pcm_output.exists());

        // Corrupt normalized inputs must never leave a published output.
        let corrupt_part = dir.join("corrupt.mkv");
        fs::write(&corrupt_part, b"not matroska").unwrap();
        let corrupt_output = dir.join("corrupt.mp4");
        assert!(execute_in_scratch(
            &engine,
            &preflight,
            &[parts[0].clone(), corrupt_part],
            &corrupt_output,
            30,
            64 * 1024 * 1024,
            || false,
        )
        .is_err());
        assert!(!corrupt_output.exists());

        // A valid encode is discarded if original sources change after encode.
        let changed_output = dir.join("changed-after-encode.mp4");
        let verify_calls = std::cell::Cell::new(0_u32);
        let changed = execute_with_source_verification(
            CrossfadeExecution {
                engine: &engine,
                plan: &preflight,
                inputs: &parts,
                output: &changed_output,
                fps: 30,
                max_bytes: 64 * 1024 * 1024,
            },
            || {
                verify_calls.set(verify_calls.get() + 1);
                if verify_calls.get() == 2 {
                    Err("Original media changed during rendering.".into())
                } else {
                    Ok(())
                }
            },
            || false,
        );
        assert!(changed.is_err());
        assert_eq!(verify_calls.get(), 2);
        assert!(!changed_output.exists());

        let rejected_output = dir.join("rejected-before-encode.mp4");
        assert!(execute_with_source_verification(
            CrossfadeExecution {
                engine: &engine,
                plan: &preflight,
                inputs: &parts,
                output: &rejected_output,
                fps: 30,
                max_bytes: 64 * 1024 * 1024,
            },
            || Err("Original source already changed.".into()),
            || false,
        )
        .is_err());
        assert!(!rejected_output.exists());

        // End-to-end experimental pipeline: prior original-source fingerprints,
        // normalized input checks, verified render/decode, exact AAC clock,
        // and staged no-overwrite publication all have to succeed.
        let origin_a = dir.join("original-red.mov");
        let origin_b = dir.join("original-blue.mov");
        fs::write(&origin_a, b"original red source").unwrap();
        fs::write(&origin_b, b"original blue source").unwrap();
        let (bytes_a, hash_a) = crate::video_project::fingerprint(&origin_a, || false).unwrap();
        let (bytes_b, hash_b) = crate::video_project::fingerprint(&origin_b, || false).unwrap();
        let original_sources = [
            crate::crossfade_integrity::SourceExpectation {
                path: &origin_a,
                bytes: bytes_a,
                sha256: &hash_a,
            },
            crate::crossfade_integrity::SourceExpectation {
                path: &origin_b,
                bytes: bytes_b,
                sha256: &hash_b,
            },
        ];
        let pipeline_output = dir.join("verified-pipeline.mp4");
        let published = dir.join("published-pipeline.mp4");
        crate::crossfade_pipeline::render_verify_publish(
            CrossfadeExecution {
                engine: &engine,
                plan: &preflight,
                inputs: &parts,
                output: &pipeline_output,
                fps: 30,
                max_bytes: 64 * 1024 * 1024,
            },
            &published,
            &original_sources,
            || false,
        )
        .unwrap();
        assert!(!pipeline_output.exists());
        assert!(published.exists());
        let published_info = video_probe::probe(&engine, &published, || false).unwrap();
        assert_eq!(published_info.frame_count, Some(105));
        let published_bytes = fs::read(&published).unwrap();

        let already_published_scratch = dir.join("already-published.mp4");
        assert!(crate::crossfade_pipeline::render_verify_publish(
            CrossfadeExecution {
                engine: &engine,
                plan: &preflight,
                inputs: &parts,
                output: &already_published_scratch,
                fps: 30,
                max_bytes: 64 * 1024 * 1024,
            },
            &published,
            &original_sources,
            || false,
        )
        .is_err());
        assert!(!already_published_scratch.exists());
        assert_eq!(fs::read(&published).unwrap(), published_bytes);

        let mutated_scratch = dir.join("mutated-original.mp4");
        fs::write(&origin_a, b"replaced red source").unwrap();
        assert!(crate::crossfade_pipeline::render_verify_publish(
            CrossfadeExecution {
                engine: &engine,
                plan: &preflight,
                inputs: &parts,
                output: &mutated_scratch,
                fps: 30,
                max_bytes: 64 * 1024 * 1024,
            },
            &dir.join("not-published.mp4"),
            &original_sources,
            || false,
        )
        .is_err());
        assert!(!mutated_scratch.exists());
        assert!(!dir.join("not-published.mp4").exists());

        // FFmpeg versions may return success when -n skips an existing file.
        // Verify the actual safety contract: output contents must not change.
        let original_bytes = fs::read(&output).unwrap();
        let _ = video_engine::run_until(
            &ffmpeg,
            &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            Duration::from_secs(90),
            || false,
        );
        assert_eq!(fs::read(&output).unwrap(), original_bytes);
    }
}
