//! Exact frame/sample seek descriptions shared by production workspace drafts and preview verification.
use crate::crossfade_layer_clock::Span as SeekSpan;
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct SeekWeight {
    pub numerator: u64,
    pub denominator: u64,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct SeekVideo {
    pub clip_id: String,
    pub asset_id: String,
    pub photo: bool,
    pub part_frame: u64,
    pub weight: SeekWeight,
    pub dip_in_frames: u64,
    pub dip_out_frames: u64,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SeekAudioOrigin {
    NormalizedClip,
    AddedTrack,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct SeekAudio {
    pub id: String,
    pub asset_id: String,
    pub origin: SeekAudioOrigin,
    pub samples: SeekSpan,
    pub source_sample_start: u64,
    pub silence: bool,
    pub gain_percent: u16,
    pub fade_in: Option<SeekSpan>,
    pub fade_out: Option<SeekSpan>,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct FrameSeek {
    pub project_frame: u64,
    pub fps: u32,
    pub samples: SeekSpan,
    pub video: Vec<SeekVideo>,
    pub titles: Vec<String>,
    pub audio: Vec<SeekAudio>,
}
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(crate) enum SeekPoint {
    Frame(u64),
    Seconds(f64),
}
fn seek_intersection(a: &SeekSpan, b: &SeekSpan) -> Option<SeekSpan> {
    let start = a.start.max(b.start);
    let end = a.end.min(b.end);
    (start < end).then_some(SeekSpan { start, end })
}
pub(crate) fn linear_seek_weight(
    span: &SeekSpan,
    sample: u64,
    rising: bool,
) -> Result<SeekWeight, String> {
    let duration = span
        .end
        .checked_sub(span.start)
        .filter(|n| *n > 0)
        .ok_or("Empty seek envelope")?;
    let denominator = duration.checked_mul(2).ok_or("Seek envelope overflow")?;
    let numerator = if sample < span.start {
        0
    } else if sample >= span.end {
        denominator
    } else {
        (sample - span.start) * 2 + 1
    };
    Ok(SeekWeight {
        numerator: if rising {
            numerator
        } else {
            denominator - numerator
        },
        denominator,
    })
}
impl SeekAudio {
    pub(crate) fn gain_at(&self, sample: u64) -> Result<f64, String> {
        if sample < self.samples.start || sample >= self.samples.end {
            return Err("Sample is outside the seek slice".into());
        }
        if self.silence {
            return Ok(0.0);
        }
        let mut gain = f64::from(self.gain_percent) / 100.0;
        for (span, rising) in [
            (self.fade_in.as_ref(), true),
            (self.fade_out.as_ref(), false),
        ] {
            if let Some(span) = span {
                let w = linear_seek_weight(span, sample, rising)?;
                gain *= w.numerator as f64 / w.denominator as f64;
            }
        }
        Ok(gain)
    }
}
pub(crate) fn seek(
    p: &crate::crossfade_layer_clock::Plan,
    point: SeekPoint,
) -> Result<FrameSeek, String> {
    if !matches!(p.fps, 30 | 60) || p.total_frames == 0 {
        return Err("Invalid seek project clock".into());
    }
    let frame = match point {
        SeekPoint::Frame(frame) => frame,
        SeekPoint::Seconds(seconds) => {
            if !seconds.is_finite()
                || seconds < 0.0
                || seconds >= p.total_frames as f64 / f64::from(p.fps)
            {
                return Err("Seek time is outside the project".into());
            }
            (seconds * f64::from(p.fps) + 1e-9).floor() as u64
        }
    };
    if frame >= p.total_frames {
        return Err("Seek frame is outside the project".into());
    }
    let per_frame = 48_000 / u64::from(p.fps);
    let samples = SeekSpan {
        start: frame * per_frame,
        end: (frame + 1) * per_frame,
    };
    let active = p
        .clips
        .iter()
        .filter(|c| frame >= c.frames.start && frame < c.frames.end)
        .collect::<Vec<_>>();
    if active.is_empty() || active.len() > 2 {
        return Err("Seek requires one or two active clips".into());
    }
    let mut video = Vec::with_capacity(active.len());
    for (index, c) in active.iter().enumerate() {
        let weight = if active.len() == 1 {
            SeekWeight {
                numerator: 1,
                denominator: 1,
            }
        } else {
            linear_seek_weight(
                &SeekSpan {
                    start: active[1].frames.start,
                    end: active[0].frames.end,
                },
                frame,
                index == 1,
            )?
        };
        video.push(SeekVideo {
            clip_id: c.id.clone(),
            asset_id: c.asset_id.clone(),
            photo: c.photo,
            part_frame: frame - c.frames.start,
            weight,
            dip_in_frames: c.dip_in_frames,
            dip_out_frames: c.dip_out_frames,
        });
    }
    let titles = p
        .titles
        .iter()
        .filter(|t| frame >= t.frames.start && frame < t.frames.end)
        .map(|t| t.id.clone())
        .collect();
    let mut audio = Vec::new();
    for (index, c) in p.clips.iter().enumerate() {
        let Some(slice) = seek_intersection(&samples, &c.samples) else {
            continue;
        };
        let fade_in = index.checked_sub(1).and_then(|i| {
            let end = p.clips[i].samples.end;
            (end > c.samples.start).then_some(SeekSpan {
                start: c.samples.start,
                end,
            })
        });
        let fade_out = p.clips.get(index + 1).and_then(|next| {
            (next.samples.start < c.samples.end).then_some(SeekSpan {
                start: next.samples.start,
                end: c.samples.end,
            })
        });
        audio.push(SeekAudio {
            id: c.id.clone(),
            asset_id: c.asset_id.clone(),
            origin: SeekAudioOrigin::NormalizedClip,
            source_sample_start: slice.start - c.samples.start,
            samples: slice,
            silence: !c.linked_audio,
            gain_percent: c.gain_percent,
            fade_in,
            fade_out,
        });
    }
    for t in &p.audio {
        let Some(slice) = seek_intersection(&samples, &t.samples) else {
            continue;
        };
        audio.push(SeekAudio {
            id: t.id.clone(),
            asset_id: t.asset_id.clone(),
            origin: SeekAudioOrigin::AddedTrack,
            source_sample_start: t.source_samples.start + (slice.start - t.samples.start),
            samples: slice,
            silence: t.gain_percent == 0,
            gain_percent: t.gain_percent,
            fade_in: (t.fade_in_samples > 0).then_some(SeekSpan {
                start: t.samples.start,
                end: t.samples.start + t.fade_in_samples,
            }),
            fade_out: (t.fade_out_samples > 0).then_some(SeekSpan {
                start: t.samples.end - t.fade_out_samples,
                end: t.samples.end,
            }),
        });
    }
    for slice in &audio {
        for sample in [slice.samples.start, slice.samples.end - 1] {
            let gain = slice.gain_at(sample)?;
            if !gain.is_finite() || !(0.0..=2.0).contains(&gain) {
                return Err("Invalid seek audio envelope".into());
            }
        }
    }
    Ok(FrameSeek {
        project_frame: frame,
        fps: p.fps,
        samples,
        video,
        titles,
        audio,
    })
}
