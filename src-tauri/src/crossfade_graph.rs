//! Experimental FFmpeg filter-graph planner for normalized clip parts.
//! Not wired into video_sequence_render: the active exporter stays hard-cut/dip-only.
//! A future native integration must prove audio sample counts and output validation.
use crate::crossfade_clock::Clock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilterGraph {
    pub filter_complex: String,
    pub video_output: String,
    pub audio_output: String,
    pub expected_frames: u64,
}

fn seconds(frames: u64, fps: u32) -> String {
    format!("{:.9}", frames as f64 / f64::from(fps))
}

/// Build a two-stream video/audio filter graph from already normalized clip parts.
///
/// Preconditions for later production use (not established by this function):
/// each input has identically sized YUV420p video at exactly fps, a stereo
/// 48 kHz PCM audio stream (silence included), an aligned zero-based clock
/// and exactly the clock entry frame count. This function does NOT open paths.
pub(crate) fn plan(clock: &Clock<'_>, fps: u32) -> Result<FilterGraph, String> {
    if !matches!(fps, 30 | 60) {
        return Err("Crossfades require a 30 or 60 fps project.".into());
    }
    if clock.entries.is_empty() {
        return Err("Crossfade filter graph needs at least one clip.".into());
    }
    if clock.entries[0].start_frame != 0 || clock.overlaps.len() > clock.entries.len() - 1 {
        return Err("Crossfade clock has an invalid initial frame or boundary count.".into());
    }
    let mut filters = Vec::new();
    for index in 0..clock.entries.len() {
        filters.push(format!(
            "[{index}:v:0]settb=AVTB,setpts=PTS-STARTPTS,fps={fps},format=yuv420p[v{index}]"
        ));
        filters.push(format!(
            "[{index}:a:0]aformat=sample_fmts=flt:channel_layouts=stereo,aresample=48000,asetpts=PTS-STARTPTS[a{index}]"
        ));
    }
    let mut video = "v0".to_owned();
    let mut audio = "a0".to_owned();
    let mut accumulated_frames = clock.entries[0].frames;
    for index in 1..clock.entries.len() {
        let previous = &clock.entries[index - 1];
        let next = &clock.entries[index];
        let boundary = clock
            .overlaps
            .iter()
            .find(|edge| edge.from_clip_id == previous.id && edge.to_clip_id == next.id);
        let overlap = boundary.map_or(0, |edge| edge.frames);
        if let Some(edge) = boundary {
            if edge.frames == 0
                || edge.start_frame != next.start_frame
                || edge.end_frame != previous.end_frame
                || edge.end_frame.checked_sub(edge.start_frame) != Some(edge.frames)
            {
                return Err("Crossfade overlap metadata does not match clip timing.".into());
            }
        }
        let expected_start = accumulated_frames
            .checked_sub(overlap)
            .ok_or("Crossfade overlap exceeds accumulated timeline.")?;
        if next.start_frame != expected_start
            || next.end_frame != next.start_frame.saturating_add(next.frames)
        {
            return Err("Crossfade clock entries are inconsistent.".into());
        }
        let output_video = format!("vx{index}");
        let output_audio = format!("ax{index}");
        if overlap == 0 {
            filters.push(format!(
                "[{video}][v{index}]concat=n=2:v=1:a=0[{output_video}]"
            ));
            filters.push(format!(
                "[{audio}][a{index}]concat=n=2:v=0:a=1[{output_audio}]"
            ));
        } else {
            let duration = seconds(overlap, fps);
            let offset = seconds(expected_start, fps);
            filters.push(format!(
                "[{video}][v{index}]xfade=transition=fade:duration={duration}:offset={offset}[{output_video}]"
            ));
            // Linear PCM envelopes are an explicit preliminary policy.
            // Equal-power or correlation-aware mixing needs listening tests.
            filters.push(format!(
                "[{audio}][a{index}]acrossfade=d={duration}:c1=tri:c2=tri[{output_audio}]"
            ));
        }
        video = output_video;
        audio = output_audio;
        accumulated_frames = next.end_frame;
    }
    if accumulated_frames != clock.total_frames {
        return Err("Crossfade total frame count is inconsistent.".into());
    }
    Ok(FilterGraph {
        filter_complex: filters.join(";"),
        video_output: video,
        audio_output: audio,
        expected_frames: clock.total_frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crossfade_clock::{self, Boundary, ClipClock};

    #[test]
    fn creates_one_video_and_audio_crossfade_with_correct_offset() {
        let clips = [
            ClipClock {
                id: "a",
                frames: 90,
            },
            ClipClock {
                id: "b",
                frames: 120,
            },
        ];
        let edge = [Boundary {
            from_clip_id: "a",
            to_clip_id: "b",
            frames: 15,
        }];
        let clock = crossfade_clock::compile(&clips, &edge).unwrap();
        let graph = plan(&clock, 30).unwrap();
        assert!(graph.filter_complex.contains(
            "[v0][v1]xfade=transition=fade:duration=0.500000000:offset=2.500000000[vx1]"
        ));
        assert!(graph
            .filter_complex
            .contains("[a0][a1]acrossfade=d=0.500000000:c1=tri:c2=tri[ax1]"));
        assert_eq!(graph.video_output, "vx1");
        assert_eq!(graph.audio_output, "ax1");
        assert_eq!(graph.expected_frames, 195);
        assert!(graph
            .filter_complex
            .contains("[0:v:0]settb=AVTB,setpts=PTS-STARTPTS,fps=30,format=yuv420p[v0]"));
        assert!(graph
            .filter_complex
            .contains("[1:v:0]settb=AVTB,setpts=PTS-STARTPTS,fps=30,format=yuv420p[v1]"));
    }

    #[test]
    fn hard_cuts_use_concat_and_keep_full_duration() {
        let clips = [
            ClipClock {
                id: "a",
                frames: 30,
            },
            ClipClock {
                id: "b",
                frames: 30,
            },
        ];
        let clock = crossfade_clock::compile(&clips, &[]).unwrap();
        let graph = plan(&clock, 30).unwrap();
        assert!(graph
            .filter_complex
            .contains("[v0][v1]concat=n=2:v=1:a=0[vx1]"));
        assert!(graph
            .filter_complex
            .contains("[a0][a1]concat=n=2:v=0:a=1[ax1]"));
        assert!(!graph.filter_complex.contains("xfade="));
        assert_eq!(graph.expected_frames, 60);
    }

    #[test]
    fn mixed_cut_types_use_accumulated_project_clock() {
        let clips = [
            ClipClock {
                id: "a",
                frames: 60,
            },
            ClipClock {
                id: "b",
                frames: 60,
            },
            ClipClock {
                id: "c",
                frames: 60,
            },
        ];
        let edges = [Boundary {
            from_clip_id: "b",
            to_clip_id: "c",
            frames: 15,
        }];
        let clock = crossfade_clock::compile(&clips, &edges).unwrap();
        let graph = plan(&clock, 30).unwrap();
        assert!(graph
            .filter_complex
            .contains("[v0][v1]concat=n=2:v=1:a=0[vx1]"));
        assert!(graph.filter_complex.contains(
            "[vx1][v2]xfade=transition=fade:duration=0.500000000:offset=3.500000000[vx2]"
        ));
        assert_eq!(graph.expected_frames, 165);
    }

    #[test]
    fn rejects_bad_project_fps_empty_inputs_and_tampered_clock() {
        let clock = crossfade_clock::compile(
            &[ClipClock {
                id: "a",
                frames: 30,
            }],
            &[],
        )
        .unwrap();
        assert!(plan(&clock, 24).is_err());
        assert!(plan(&crossfade_clock::compile(&[], &[]).unwrap(), 30).is_err());
        let mut invalid = clock.clone();
        invalid.total_frames += 1;
        assert!(plan(&invalid, 30).is_err());
        let mut invalid = clock.clone();
        invalid.entries[0].start_frame = 1;
        assert!(plan(&invalid, 30).is_err());
    }

    #[test]
    fn rejects_tampered_overlap_metadata_without_unsafe_filter_offsets() {
        let clips = [
            ClipClock {
                id: "a",
                frames: 90,
            },
            ClipClock {
                id: "b",
                frames: 120,
            },
        ];
        let edges = [Boundary {
            from_clip_id: "a",
            to_clip_id: "b",
            frames: 15,
        }];
        let clock = crossfade_clock::compile(&clips, &edges).unwrap();
        let mut invalid = clock.clone();
        invalid.overlaps[0].start_frame += 1;
        assert!(plan(&invalid, 30).is_err());
        let mut invalid = clock.clone();
        invalid.overlaps[0].frames = 0;
        assert!(plan(&invalid, 30).is_err());
        let mut invalid = clock.clone();
        invalid.overlaps[0].end_frame -= 1;
        assert!(plan(&invalid, 30).is_err());
    }
}

#[cfg(all(test, windows))]
mod bundled_engine_tests {
    use super::*;
    use crate::{
        crossfade_clock::{self, Boundary, ClipClock},
        video_engine, video_probe, video_project,
    };
    use std::{
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        time::Duration,
    };

    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn ffmpeg(engine: &Path, args: &[OsString]) {
        video_engine::run(
            &video_engine::tool(engine, "ffmpeg").unwrap(),
            &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
            Duration::from_secs(90),
        )
        .unwrap();
    }

    #[test]
    fn bundled_ffmpeg_renders_half_second_audio_video_overlap() {
        let engine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine");
        let dir = std::env::temp_dir().join(format!("crossfade-graph-{}", video_project::id()));
        fs::create_dir(&dir).unwrap();
        let _fixture = Fixture(dir.clone());

        let inputs = [("red", 90_u64, 3_u32), ("blue", 120_u64, 4_u32)];
        for (index, (color, frames, seconds)) in inputs.iter().enumerate() {
            let file = dir.join(format!("clip-{index}.mkv"));
            let args: Vec<OsString> = vec![
                "-nostdin".into(),
                "-v".into(),
                "error".into(),
                "-f".into(),
                "lavfi".into(),
                "-i".into(),
                format!("color=c={color}:s=96x64:r=30:d={seconds}").into(),
                "-f".into(),
                "lavfi".into(),
                "-i".into(),
                "anullsrc=r=48000:cl=stereo".into(),
                "-t".into(),
                seconds.to_string().into(),
                "-frames:v".into(),
                frames.to_string().into(),
                "-c:v".into(),
                "libopenh264".into(),
                "-b:v".into(),
                "150000".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
                "-c:a".into(),
                "pcm_f32le".into(),
                file.into(),
            ];
            ffmpeg(&engine, &args);
        }

        let clips = [
            ClipClock {
                id: "red",
                frames: 90,
            },
            ClipClock {
                id: "blue",
                frames: 120,
            },
        ];
        let edge = [Boundary {
            from_clip_id: "red",
            to_clip_id: "blue",
            frames: 15,
        }];
        let clock = crossfade_clock::compile(&clips, &edge).unwrap();
        let graph = plan(&clock, 30).unwrap();
        let output = dir.join("crossfade.mp4");
        let args: Vec<OsString> = vec![
            "-nostdin".into(),
            "-v".into(),
            "error".into(),
            "-i".into(),
            dir.join("clip-0.mkv").into(),
            "-i".into(),
            dir.join("clip-1.mkv").into(),
            "-filter_complex".into(),
            graph.filter_complex.into(),
            "-map".into(),
            format!("[{}]", graph.video_output).into(),
            "-map".into(),
            format!("[{}]", graph.audio_output).into(),
            "-c:v".into(),
            "libopenh264".into(),
            "-b:v".into(),
            "200000".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-c:a".into(),
            "aac".into(),
            "-ar".into(),
            "48000".into(),
            "-ac".into(),
            "2".into(),
            output.clone().into(),
        ];
        ffmpeg(&engine, &args);
        let info = video_probe::probe(&engine, &output, || false).unwrap();
        assert_eq!(info.frame_count, Some(graph.expected_frames));
        assert_eq!((info.width, info.height), (96, 64));
        assert!((info.duration_seconds - 6.5).abs() < 0.05);
        assert_eq!(info.audio.len(), 1);
        assert_eq!(info.audio[0].sample_rate.as_deref(), Some("48000"));
        let audio_seconds = info.audio[0]
            .duration_seconds
            .unwrap_or(info.duration_seconds);
        assert!((audio_seconds - 6.5).abs() < 0.05);
    }

    #[test]
    fn bundled_ffmpeg_renders_two_consecutive_crossfades_without_clock_drift() {
        let engine = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/video-engine");
        let dir = std::env::temp_dir().join(format!("crossfade-chain-{}", video_project::id()));
        fs::create_dir(&dir).unwrap();
        let _fixture = Fixture(dir.clone());

        for (index, color) in ["red", "green", "blue"].iter().enumerate() {
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
                dir.join(format!("clip-{index}.mkv")).into(),
            ];
            ffmpeg(&engine, &args);
        }

        let clips = [
            ClipClock {
                id: "a",
                frames: 60,
            },
            ClipClock {
                id: "b",
                frames: 60,
            },
            ClipClock {
                id: "c",
                frames: 60,
            },
        ];
        let edges = [
            Boundary {
                from_clip_id: "a",
                to_clip_id: "b",
                frames: 15,
            },
            Boundary {
                from_clip_id: "b",
                to_clip_id: "c",
                frames: 15,
            },
        ];
        let clock = crossfade_clock::compile(&clips, &edges).unwrap();
        let graph = plan(&clock, 30).unwrap();
        assert_eq!(graph.expected_frames, 150);
        assert!(graph.filter_complex.contains(
            "[v0][v1]xfade=transition=fade:duration=0.500000000:offset=1.500000000[vx1]"
        ));
        assert!(graph.filter_complex.contains(
            "[vx1][v2]xfade=transition=fade:duration=0.500000000:offset=3.000000000[vx2]"
        ));

        let output = dir.join("chained-crossfades.mp4");
        let args: Vec<OsString> = vec![
            "-nostdin".into(),
            "-v".into(),
            "error".into(),
            "-i".into(),
            dir.join("clip-0.mkv").into(),
            "-i".into(),
            dir.join("clip-1.mkv").into(),
            "-i".into(),
            dir.join("clip-2.mkv").into(),
            "-filter_complex".into(),
            graph.filter_complex.into(),
            "-map".into(),
            format!("[{}]", graph.video_output).into(),
            "-map".into(),
            format!("[{}]", graph.audio_output).into(),
            "-c:v".into(),
            "libopenh264".into(),
            "-b:v".into(),
            "200000".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-c:a".into(),
            "aac".into(),
            "-ar".into(),
            "48000".into(),
            "-ac".into(),
            "2".into(),
            output.clone().into(),
        ];
        ffmpeg(&engine, &args);
        let info = video_probe::probe(&engine, &output, || false).unwrap();
        assert_eq!(info.frame_count, Some(graph.expected_frames));
        assert!((info.duration_seconds - 5.0).abs() < 0.05);
        assert_eq!((info.width, info.height), (96, 64));
        assert_eq!(info.audio.len(), 1);
        assert_eq!(info.audio[0].sample_rate.as_deref(), Some("48000"));
        let audio_seconds = info.audio[0]
            .duration_seconds
            .unwrap_or(info.duration_seconds);
        assert!((audio_seconds - 5.0).abs() < 0.05);
    }
}
