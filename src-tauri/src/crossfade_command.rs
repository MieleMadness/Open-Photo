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
