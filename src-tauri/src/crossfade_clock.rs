//! Future true crossfade frame-clock contract. Not connected to the active renderer.
//! Source and project frames stay integer-valued until presentation.
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClipClock<'a> {
    pub id: &'a str,
    pub frames: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Boundary<'a> {
    pub from_clip_id: &'a str,
    pub to_clip_id: &'a str,
    pub frames: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry<'a> {
    pub id: &'a str,
    pub start_frame: u64,
    pub end_frame: u64,
    pub frames: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Overlap<'a> {
    pub from_clip_id: &'a str,
    pub to_clip_id: &'a str,
    pub start_frame: u64,
    pub end_frame: u64,
    pub frames: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Clock<'a> {
    pub entries: Vec<Entry<'a>>,
    pub overlaps: Vec<Overlap<'a>>,
    pub total_frames: u64,
}

/// The same adjacency, two-clean-frames and total-frame policy as crossfadeClock.ts.
/// No schema/rendering changes are made by calling this test-only foundation.
pub(crate) fn compile<'a>(
    clips: &[ClipClock<'a>],
    edges: &[Boundary<'a>],
) -> Result<Clock<'a>, String> {
    let mut ids = HashSet::new();
    for clip in clips {
        if clip.id.is_empty()
            || clip.id.len() > 120
            || !clip
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !ids.insert(clip.id)
            || clip.frames < 2
        {
            return Err("Crossfade clips require unique IDs and at least two frames.".into());
        }
    }
    if edges.len() > clips.len().saturating_sub(1) {
        return Err("Too many crossfade boundaries.".into());
    }
    let mut bounds = vec![0_u64; clips.len().saturating_sub(1)];
    for edge in edges {
        let cut = clips
            .windows(2)
            .position(|pair| pair[0].id == edge.from_clip_id && pair[1].id == edge.to_clip_id);
        let Some(cut) = cut else {
            return Err("A crossfade must join adjacent clips exactly once.".into());
        };
        if bounds[cut] != 0 {
            return Err("A crossfade must join adjacent clips exactly once.".into());
        }
        if edge.frames < 1 {
            return Err("Crossfade duration must contain whole project frames.".into());
        }
        bounds[cut] = edge.frames;
    }
    for (index, clip) in clips.iter().enumerate() {
        let incoming = if index == 0 { 0 } else { bounds[index - 1] };
        let outgoing = bounds.get(index).copied().unwrap_or(0);
        if incoming
            .checked_add(outgoing)
            .and_then(|sum| sum.checked_add(2))
            .is_none_or(|required| required > clip.frames)
        {
            return Err("Crossfade duration exceeds the available clip frames.".into());
        }
    }
    let mut entries = Vec::with_capacity(clips.len());
    let mut overlaps = Vec::with_capacity(bounds.len());
    let mut previous_end = 0_u64;
    for (index, clip) in clips.iter().enumerate() {
        let overlap = if index == 0 { 0 } else { bounds[index - 1] };
        let start = previous_end
            .checked_sub(overlap)
            .ok_or("Invalid overlap clock")?;
        let end = start
            .checked_add(clip.frames)
            .ok_or("Crossfade timeline exceeds the safe frame range.")?;
        if overlap > 0 {
            overlaps.push(Overlap {
                from_clip_id: clips[index - 1].id,
                to_clip_id: clip.id,
                start_frame: start,
                end_frame: previous_end,
                frames: overlap,
            });
        }
        entries.push(Entry {
            id: clip.id,
            start_frame: start,
            end_frame: end,
            frames: clip.frames,
        });
        previous_end = end;
    }
    Ok(Clock {
        entries,
        overlaps,
        total_frames: previous_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_typescript_single_overlap_clock() {
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
        let clock = compile(&clips, &edges).unwrap();
        assert_eq!(clock.total_frames, 195);
        assert_eq!(clock.entries[0].start_frame, 0);
        assert_eq!(clock.entries[0].end_frame, 90);
        assert_eq!(clock.entries[1].start_frame, 75);
        assert_eq!(clock.entries[1].end_frame, 195);
        assert_eq!(clock.overlaps[0].start_frame, 75);
        assert_eq!(clock.overlaps[0].end_frame, 90);
    }

    #[test]
    fn matches_typescript_two_cut_clock_and_hard_cut_fallback() {
        let clips = [
            ClipClock {
                id: "a",
                frames: 60,
            },
            ClipClock {
                id: "b",
                frames: 50,
            },
            ClipClock {
                id: "c",
                frames: 70,
            },
        ];
        let edges = [
            Boundary {
                from_clip_id: "a",
                to_clip_id: "b",
                frames: 12,
            },
            Boundary {
                from_clip_id: "b",
                to_clip_id: "c",
                frames: 18,
            },
        ];
        let clock = compile(&clips, &edges).unwrap();
        assert_eq!(clock.total_frames, 150);
        assert_eq!(clock.entries[1].start_frame, 48);
        assert_eq!(clock.entries[1].end_frame, 98);
        assert_eq!(clock.entries[2].start_frame, 80);
        assert_eq!(clock.overlaps[1].end_frame, 98);
        assert_eq!(compile(&clips, &[]).unwrap().total_frames, 180);
        assert!(compile(&[], &[]).unwrap().entries.is_empty());
    }

    #[test]
    fn rejects_exhausted_adjacency_duplicate_and_overflow() {
        let clips = [
            ClipClock {
                id: "a",
                frames: 30,
            },
            ClipClock {
                id: "b",
                frames: 20,
            },
            ClipClock {
                id: "c",
                frames: 30,
            },
        ];
        let first = Boundary {
            from_clip_id: "a",
            to_clip_id: "b",
            frames: 10,
        };
        assert!(compile(&clips, &[first.clone(), first.clone()]).is_err());
        assert!(compile(
            &clips,
            &[Boundary {
                from_clip_id: "a",
                to_clip_id: "c",
                frames: 2
            }]
        )
        .is_err());
        assert!(compile(
            &clips,
            &[
                first.clone(),
                Boundary {
                    from_clip_id: "b",
                    to_clip_id: "c",
                    frames: 9
                }
            ]
        )
        .is_err());
        assert!(compile(&clips, &[Boundary { frames: 0, ..first }]).is_err());
        assert!(compile(
            &[
                ClipClock {
                    id: "a",
                    frames: u64::MAX
                },
                ClipClock {
                    id: "b",
                    frames: 10
                }
            ],
            &[]
        )
        .is_err());
    }
}
