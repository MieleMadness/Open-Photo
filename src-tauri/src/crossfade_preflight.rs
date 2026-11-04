//! Test-only preflight for an eventual native crossfade sequence renderer.
//! Existing hard-cut/dip preview/export and saved v3 project semantics are untouched.
use crate::{
    crossfade_clock::{self, Boundary, ClipClock},
    crossfade_graph::{self, FilterGraph},
    sequence_media::Media,
    video_sequence::{self, Sequence},
    video_sequence_render,
};

#[derive(Debug)]
pub(crate) struct PreparedCrossfade {
    pub graph: FilterGraph,
    pub normalized_clip_frames: Vec<u64>,
    pub silence_required_for: Vec<usize>,
    pub expected_audio_samples: u64,
}

/// Check frame allocations and overlap requirements **before opening sources**.
/// The returned silence indices are a requirement for a future normalizer, not
/// proof that audio streams have been normalized or that export is ready.
pub(crate) fn plan_allocated(
    clips: &[ClipClock<'_>],
    boundaries: &[Boundary<'_>],
    source_has_audio: &[bool],
    fps: u32,
) -> Result<PreparedCrossfade, String> {
    if !matches!(fps, 30 | 60) {
        return Err("Crossfades require a 30 or 60 fps project.".into());
    }
    if clips.len() < 2 || clips.len() > 200 || source_has_audio.len() != clips.len() {
        return Err("Crossfade preflight requires 2–200 clips and matching audio metadata.".into());
    }
    if boundaries.is_empty() {
        return Err("Crossfade preflight needs at least one overlapping boundary.".into());
    }
    let clock = crossfade_clock::compile(clips, boundaries)?;
    if clock.total_frames > u64::from(fps) * 7200 {
        return Err("Crossfade project duration exceeds two hours.".into());
    }
    let graph = crossfade_graph::plan(&clock, fps)?;
    let expected_audio_samples = graph
        .expected_frames
        .checked_mul(48_000)
        .ok_or("Crossfade audio sample count overflow.")?
        / u64::from(fps);
    Ok(PreparedCrossfade {
        graph,
        normalized_clip_frames: clips.iter().map(|c| c.frames).collect(),
        silence_required_for: source_has_audio
            .iter()
            .enumerate()
            .filter_map(|(index, has_audio)| (!*has_audio).then_some(index))
            .collect(),
        expected_audio_samples,
    })
}

/// Construct a safe candidate plan from existing saved clip marks, with
/// hypothetical overlap boundaries supplied separately. Never edits v3 state.
pub(crate) fn plan_sequence(
    project: &Sequence,
    boundaries: &[Boundary<'_>],
) -> Result<PreparedCrossfade, String> {
    video_sequence::validate(project)?;
    if !project.transitions.is_empty()
        || !project.titles.is_empty()
        || !project.audio_tracks.is_empty()
        || !project.audio_sources.is_empty()
    {
        return Err(
            "Experimental crossfades cannot be combined with saved dips, titles or extra audio."
                .into(),
        );
    }
    let frames = video_sequence_render::frame_allocation(project)?;
    let mut sources = Vec::with_capacity(project.clips.len());
    let mut has_audio = Vec::with_capacity(project.clips.len());
    for (clip, frames) in project.clips.iter().zip(frames.iter()) {
        let asset = project
            .assets
            .iter()
            .find(|asset| asset.id == clip.asset_id)
            .ok_or("Relink a missing source asset.")?;
        let Media::Video { info } = &asset.media else {
            return Err("Experimental crossfades do not yet support photos.".into());
        };
        sources.push(ClipClock {
            id: &clip.id,
            frames: *frames,
        });
        has_audio.push(!info.audio.is_empty() && !clip.edits.muted && clip.edits.gain_percent > 0);
    }
    plan_allocated(&sources, boundaries, &has_audio, project.frame_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_two_stream_crossfade_with_exact_audio_samples() {
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
        let p = plan_allocated(&clips, &edge, &[true, false], 30).unwrap();
        assert_eq!(p.graph.expected_frames, 195);
        assert_eq!(p.expected_audio_samples, 312_000);
        assert_eq!(p.silence_required_for, vec![1]);
        assert_eq!(p.normalized_clip_frames, vec![90, 120]);
        assert!(p.graph.filter_complex.contains("xfade=transition=fade"));
    }

    #[test]
    fn plans_consecutive_crossfades_and_mixed_audio_presence_at_60fps() {
        let clips = [
            ClipClock {
                id: "a",
                frames: 120,
            },
            ClipClock {
                id: "b",
                frames: 120,
            },
            ClipClock {
                id: "c",
                frames: 120,
            },
        ];
        let edges = [
            Boundary {
                from_clip_id: "a",
                to_clip_id: "b",
                frames: 30,
            },
            Boundary {
                from_clip_id: "b",
                to_clip_id: "c",
                frames: 30,
            },
        ];
        let p = plan_allocated(&clips, &edges, &[false, true, false], 60).unwrap();
        assert_eq!(p.graph.expected_frames, 300);
        assert_eq!(p.expected_audio_samples, 240_000);
        assert_eq!(p.silence_required_for, vec![0, 2]);
    }

    #[test]
    fn refuses_missing_metadata_empty_overlaps_and_exhausted_source() {
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
        let edges = [Boundary {
            from_clip_id: "a",
            to_clip_id: "b",
            frames: 10,
        }];
        assert!(plan_allocated(&clips, &edges, &[true], 30).is_err());
        assert!(plan_allocated(&clips, &[], &[true, true], 30).is_err());
        assert!(plan_allocated(&clips, &edges, &[true, true], 24).is_err());
        let exhausted = [Boundary {
            from_clip_id: "a",
            to_clip_id: "b",
            frames: 29,
        }];
        assert!(plan_allocated(&clips, &exhausted, &[true, true], 30).is_err());
    }

    #[test]
    fn rejects_frame_limit_before_emitting_a_graph() {
        let clips = [
            ClipClock {
                id: "a",
                frames: 150_000,
            },
            ClipClock {
                id: "b",
                frames: 90_000,
            },
        ];
        let edges = [Boundary {
            from_clip_id: "a",
            to_clip_id: "b",
            frames: 10,
        }];
        assert!(plan_allocated(&clips, &edges, &[true, true], 30).is_err());
    }

    #[test]
    fn accepts_exact_two_hour_limit_and_rejects_one_extra_frame() {
        let clips = [
            ClipClock {
                id: "a",
                frames: 108_001,
            },
            ClipClock {
                id: "b",
                frames: 108_001,
            },
        ];
        let edges = [Boundary {
            from_clip_id: "a",
            to_clip_id: "b",
            frames: 2,
        }];
        let accepted = plan_allocated(&clips, &edges, &[false, false], 30).unwrap();
        assert_eq!(accepted.graph.expected_frames, 216_000);
        assert_eq!(accepted.expected_audio_samples, 345_600_000);
        assert_eq!(accepted.silence_required_for, vec![0, 1]);

        let over_limit = [
            ClipClock {
                id: "a",
                frames: 108_002,
            },
            ClipClock {
                id: "b",
                frames: 108_001,
            },
        ];
        assert!(plan_allocated(&over_limit, &edges, &[false, false], 30).is_err());
    }

    #[test]
    fn refuses_more_than_two_hundred_clips_before_graph_planning() {
        let ids: Vec<String> = (0..201).map(|index| format!("clip-{index}")).collect();
        let clips: Vec<ClipClock<'_>> = ids
            .iter()
            .map(|id| ClipClock {
                id: id.as_str(),
                frames: 30,
            })
            .collect();
        let boundaries = [Boundary {
            from_clip_id: "clip-0",
            to_clip_id: "clip-1",
            frames: 2,
        }];
        assert!(plan_allocated(&clips, &boundaries, &[true; 201], 30).is_err());
        let accepted = plan_allocated(&clips[..200], &boundaries, &[true; 200], 30).unwrap();
        assert_eq!(accepted.graph.expected_frames, 5_998);
        assert!(accepted.silence_required_for.is_empty());
    }

    #[test]
    fn rejects_invalid_saved_sequence_before_computing_crossfades() {
        let project = Sequence {
            schema_version: 3,
            id: "project-1".into(),
            title: "Invalid source-less project".into(),
            assets: vec![],
            clips: vec![],
            output: crate::video_project::Output {
                width: 96,
                height: 64,
                fit: "fit".into(),
            },
            frame_rate: 30,
            titles: vec![],
            audio_tracks: vec![],
            audio_sources: vec![],
            transitions: vec![],
        };
        assert!(plan_sequence(&project, &[]).is_err());
    }
}
