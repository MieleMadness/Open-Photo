//! Proposed mixed-media/layer timing contract, compiled only for tests.
//! No rendering, persistence, source reads or production command registration.
use crate::{
    crossfade_clock::{self, Boundary, ClipClock},
    sequence_media::Media,
    video_sequence::{self, Sequence},
    video_sequence_render,
};

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Span {
    pub start: u64,
    pub end: u64,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClipPlan {
    pub id: String,
    pub asset_id: String,
    pub photo: bool,
    pub linked_audio: bool,
    pub gain_percent: u16,
    pub frames: Span,
    pub samples: Span,
    pub dip_in_frames: u64,
    pub dip_out_frames: u64,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TitlePlan {
    pub id: String,
    pub frames: Span,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AudioPlan {
    pub id: String,
    pub asset_id: String,
    pub samples: Span,
    pub source_samples: Span,
    pub fade_in_samples: u64,
    pub fade_out_samples: u64,
    pub gain_percent: u16,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Plan {
    pub fps: u32,
    pub total_frames: u64,
    pub total_samples: u64,
    pub clips: Vec<ClipPlan>,
    pub titles: Vec<TitlePlan>,
    pub audio: Vec<AudioPlan>,
}
fn mark(seconds: f64, rate: u32) -> Result<u64, String> {
    let value = seconds * f64::from(rate);
    if !value.is_finite() || !(0.0..=9_007_199_254_740_991.0).contains(&value) {
        return Err("Unsafe layer timestamp".into());
    }
    Ok(value.round() as u64)
}
fn range(start: f64, end: f64, rate: u32, total: u64) -> Result<Span, String> {
    let span = Span {
        start: mark(start, rate)?,
        end: mark(end, rate)?,
    };
    if span.start >= span.end || span.end > total {
        return Err(
            "Layer timing collapses or exceeds the shortened timeline; edit it explicitly".into(),
        );
    }
    Ok(span)
}
pub(crate) fn plan(project: &Sequence, edges: &[Boundary<'_>]) -> Result<Plan, String> {
    video_sequence::validate(project)?;
    if project.clips.is_empty() {
        return Err("Overlap planning requires a nonempty timeline".into());
    }
    if project
        .audio_sources
        .iter()
        .any(|source| project.assets.iter().any(|asset| asset.id == source.id))
    {
        return Err("Independent audio and media sources need unambiguous identities".into());
    }
    let allocations = video_sequence_render::frame_allocation(project)?;
    let clips = project
        .clips
        .iter()
        .zip(allocations)
        .map(|(c, frames)| ClipClock { id: &c.id, frames })
        .collect::<Vec<_>>();
    let clock = crossfade_clock::compile(&clips, edges)?;
    let samples_per_frame = 48_000 / u64::from(project.frame_rate);
    let total_samples = clock
        .total_frames
        .checked_mul(samples_per_frame)
        .ok_or("Unsafe sample clock")?;
    let mut plans = Vec::with_capacity(clips.len());
    for (index, (clip, entry)) in project.clips.iter().zip(&clock.entries).enumerate() {
        let asset = project
            .assets
            .iter()
            .find(|a| a.id == clip.asset_id)
            .ok_or("Missing clip source")?;
        // A dip uses its existing local half-duration at either side of a cut;
        // ceil reserves every frame touched by the continuous fade envelope.
        let dip_in = project
            .transitions
            .iter()
            .find(|t| t.to_clip_id == clip.id)
            .map_or(0, |t| {
                (t.duration_seconds * f64::from(project.frame_rate) / 2.0).ceil() as u64
            });
        let dip_out = project
            .transitions
            .iter()
            .find(|t| t.from_clip_id == clip.id)
            .map_or(0, |t| {
                (t.duration_seconds * f64::from(project.frame_rate) / 2.0).ceil() as u64
            });
        let incoming = clock
            .overlaps
            .iter()
            .find(|e| e.to_clip_id == clip.id)
            .map_or(0, |e| e.frames);
        let outgoing = clock
            .overlaps
            .iter()
            .find(|e| e.from_clip_id == clip.id)
            .map_or(0, |e| e.frames);
        if (dip_in > 0 && incoming > 0) || (dip_out > 0 && outgoing > 0) {
            return Err("Choose either a dip or a crossfade at the same boundary".into());
        }
        if incoming + outgoing + dip_in + dip_out + 2 > entry.frames {
            return Err(format!(
                "Clip {} needs two clear frames between transition envelopes",
                index + 1
            ));
        }
        let linked_audio = match &asset.media {
            Media::Video { info } => {
                !info.audio.is_empty() && !clip.edits.muted && clip.edits.gain_percent > 0
            }
            Media::Photo { .. } => false,
        };
        plans.push(ClipPlan {
            id: clip.id.clone(),
            asset_id: asset.id.clone(),
            photo: asset.is_photo(),
            linked_audio,
            gain_percent: clip.edits.gain_percent,
            frames: Span {
                start: entry.start_frame,
                end: entry.end_frame,
            },
            samples: Span {
                start: entry.start_frame * samples_per_frame,
                end: entry.end_frame * samples_per_frame,
            },
            dip_in_frames: dip_in,
            dip_out_frames: dip_out,
        });
    }
    // Titles are project-global overlays after transition composition. They
    // appear once through an overlap; they are not baked into both clip parts.
    let titles = project
        .titles
        .iter()
        .map(|t| {
            Ok(TitlePlan {
                id: t.id.clone(),
                frames: range(
                    t.start_seconds,
                    t.end_seconds,
                    project.frame_rate,
                    clock.total_frames,
                )?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    // Added tracks are mixed once after linked-clip crossfades, at 48 kHz.
    // They do not inherit clip overlap envelopes or ripple with downstream cuts.
    let audio = project
        .audio_tracks
        .iter()
        .map(|t| {
            let samples = range(
                t.start_seconds,
                t.start_seconds + t.duration_seconds,
                48_000,
                total_samples,
            )?;
            let source_start = mark(t.source_in_seconds, 48_000)?;
            let source_end = source_start
                .checked_add(samples.end - samples.start)
                .ok_or("Unsafe audio source clock")?;
            let source_duration =
                if let Some(a) = project.audio_sources.iter().find(|a| a.id == t.asset_id) {
                    a.info.duration_seconds
                } else {
                    match &project
                        .assets
                        .iter()
                        .find(|a| a.id == t.asset_id)
                        .ok_or("Missing audio source")?
                        .media
                    {
                        Media::Video { info } => info.duration_seconds,
                        Media::Photo { .. } => return Err("Photos have no audio stream".into()),
                    }
                };
            let fade_in = mark(t.fade_in_seconds, 48_000)?;
            let fade_out = mark(t.fade_out_seconds, 48_000)?;
            if source_end > mark(source_duration, 48_000)?
                || fade_in + fade_out > samples.end - samples.start
            {
                return Err("Audio source or fades exceed the quantized track span".into());
            }
            Ok(AudioPlan {
                id: t.id.clone(),
                asset_id: t.asset_id.clone(),
                samples,
                source_samples: Span {
                    start: source_start,
                    end: source_end,
                },
                fade_in_samples: fade_in,
                fade_out_samples: fade_out,
                gain_percent: t.gain_percent,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Plan {
        fps: project.frame_rate,
        total_frames: clock.total_frames,
        total_samples,
        clips: plans,
        titles,
        audio,
    })
}
