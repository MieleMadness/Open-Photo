# True crossfades — Phase I frame-clock groundwork

Implementation branch: `feature/crossfade-timeline-clock-foundation`. Follow-up to [issue #20](https://github.com/MieleMadness/Open-Photo/issues/20).

## What this patch does

Introduces `src/crossfadeClock.ts`, a pure and testable integer-frame timing model. It is **not yet connected** to `SequenceProject`, the UI, or native FFmpeg. The current hard-cut and `dip_to_black` output remains unchanged.

Each clip owns its normal integer frame count. A crossfade of `N` frames starts the next clip `N` frames before the previous clip ends. Therefore the output clock is `sum(clipFrames) - sum(overlapFrames)`; two adjacent source streams contribute frames during each overlap. This differs from a dip-to-black, which preserves the cut instant and overlays no footage.

## Safety constraints

- Overlap boundaries must point to immediately adjacent, uniquely identified clips.
- Each overlap must be an integer number of project frames.
- Incoming plus outgoing overlaps cannot consume a clip: reserve at least two non-overlapping frames per clip. This avoids accidental three-way overlap on an interior clip.
- Saved source marks are not moved or rewritten by the timing computation.
- Original assets, storage schema, native renderer, exports, and audio are untouched.

## Critical decisions before activation

1. **Source frame policy:** Initial implementation should overlap the tail of outgoing kept range with the head of incoming kept range (without requesting hidden source handles). Preview project clock must explicitly shorten. Do not quietly extend into portions the user trimmed away.
2. **Linked audio:** Concurrent audio in overlap must be mixed using equal-power or linear gain envelopes, with a documented default and silence/mute handling; do not assume current source-audio concatenation naturally crossfades.
3. **Photos:** Virtual photo frames can participate once the native two-source compositor has mixed-media validation; keep initial cut type limited until tested.
4. **Titles and standalone music:** Their timeline positions need an explicit migration/adjustment policy when project duration changes. Existing saved hard-cut projects should remain unchanged.
5. **Persistence and migrations:** Introduce versioned transition metadata and a matching Rust validator with strict backward compatibility. This prototype must not be interpreted as support for crossfade project documents.
6. **Fixture parity:** Compare actual preview/export frames at every overlap segment, video duration, linked audio and standalone tracks. Preserve no-overwrite, source fingerprints, cancellation and cleanup.

## Acceptance

Pure timing tests cover hard cuts, one and multiple overlapping pairs, adjacent validation, non-overlapping guards, integer-frame and time calculations. Native implementation, audio mixing, UI interaction, installed Windows review and full release CI are still required before claiming that users can create true crossfades.
