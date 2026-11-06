// Test-only seek descriptions over a freshly verified selected manifest.
// No source decoding, playback, mixing or production preview route.
use crate::crossfade_layer_clock::Span as SeekSpan;
#[derive(Debug, PartialEq, Eq)]
struct SeekWeight {
    numerator: u64,
    denominator: u64,
}
#[derive(Debug, PartialEq, Eq)]
struct SeekVideo {
    clip_id: String,
    asset_id: String,
    photo: bool,
    part_frame: u64,
    weight: SeekWeight,
    dip_in_frames: u64,
    dip_out_frames: u64,
}
#[derive(Debug, PartialEq, Eq)]
enum SeekAudioOrigin {
    NormalizedClip,
    AddedTrack,
}
#[derive(Debug, PartialEq, Eq)]
struct SeekAudio {
    id: String,
    asset_id: String,
    origin: SeekAudioOrigin,
    samples: SeekSpan,
    source_sample_start: u64,
    silence: bool,
    gain_percent: u16,
    fade_in: Option<SeekSpan>,
    fade_out: Option<SeekSpan>,
}
#[derive(Debug, PartialEq, Eq)]
struct SessionSeek {
    session: SessionReceipt,
    project_revision: u64,
    document_hash: String,
    project_frame: u64,
    fps: u32,
    samples: SeekSpan,
    video: Vec<SeekVideo>,
    titles: Vec<String>,
    audio: Vec<SeekAudio>,
}
#[derive(Clone, Copy)]
enum SeekPoint {
    Frame(u64),
    Seconds(f64),
}
fn seek_intersection(a: &SeekSpan, b: &SeekSpan) -> Option<SeekSpan> {
    let start = a.start.max(b.start);
    let end = a.end.min(b.end);
    (start < end).then_some(SeekSpan { start, end })
}
fn linear_seek_weight(span: &SeekSpan, sample: u64, rising: bool) -> Result<SeekWeight, String> {
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
    fn gain_at(&self, sample: u64) -> Result<f64, String> {
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
// Called only with the independently revalidated manifest inside one read tx.
fn seek_verified_manifest(
    s: &EditorSession,
    m: &LayerManifest,
    point: SeekPoint,
) -> Result<SessionSeek, String> {
    let p = &m.timing;
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
    Ok(SessionSeek {
        session: s.receipt.clone(),
        project_revision: m.project_revision,
        document_hash: m.document_hash.clone(),
        project_frame: frame,
        fps: p.fps,
        samples,
        video,
        titles,
        audio,
    })
}
fn seek_session(
    db: &mut Connection,
    s: &EditorSession,
    expected: &SessionReceipt,
    point: SeekPoint,
) -> Result<SessionSeek, String> {
    if expected != &s.receipt {
        return Err("Session changed before seeking; reload its receipt".into());
    }
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let manifest = load_layer_manifest_in(&tx, &s.root, &s.project_id, &s.receipt.current_id)?;
    let position = seek_verified_manifest(s, &manifest, point)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(position)
}
