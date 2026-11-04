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
    let mut filters = Vec::new();
    for index in 0..clock.entries.len() {
        filters.push(format!(
            "[{index}:v:0]settb=AVTB,setpts=PTS-STARTPTS,format=yuv420p[v{index}]"
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
        assert!(graph.filter_complex.contains("[v0][v1]concat=n=2:v=1:a=0[vx1]"));
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
    }
}
