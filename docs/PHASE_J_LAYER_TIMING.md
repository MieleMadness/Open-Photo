# Phase J mixed-media and layer timing — local 0.56.79

This update makes the proposed timing rules executable against native schema-v3
projects. `crossfade_layer_clock` is compiled only under `cfg(test)`. A separate
read-only adapter binds a plan to the real saved root/project, revision and
whole stored-document hash in one SQLite transaction. It does not save layer
plans, change production project formats, render media or register commands.

## Proposed timing rules

Video and photo clips use the current renderer's cumulative native frame
allocation at 30 or 60 fps. The existing crossfade clock subtracts overlaps from
total duration without changing source trims or photo geometry. Every clip
retains at least two clear frames after its incoming/outgoing transition
envelopes. A photo occupies its saved duration and contributes silence to linked
audio; no video/audio metadata is invented for it. Still-only sequences use the
same rules.

Titles keep their absolute project times and are planned as **one overlay after
transition composition**. They are not duplicated in both overlapping clip
parts. This proposes that global titles remain legible through dips/crossfades;
the current production renderer bakes titles into parts before its dip fade.
This proposed compositor order requires preview/export review before production
integration and does not change today's renderer.

Title endpoints round independently to the nearest project frame, with positive
half ties rounding up. All spans are half-open `[start,end)`. A collapsed range
or endpoint past the shortened frame total fails explicitly. Nothing moves,
truncates or stretches a title automatically. Subframe behavior is a proposed
quantization policy, not a claim of parity with today's continuous title filter.

Linked clip audio follows the same overlapping project clock at 48 kHz: exactly
1,600 samples per 30 fps frame or 800 per 60 fps frame. Saved stream presence,
mute and gain determine whether a clip contributes linked audio; photos,
missing audio streams, muted clips and zero gain require silence. Existing
experimental linear crossfade envelopes remain a separate rendering/listening
gate. Dips remain video-only and do not shorten the cut or fade audio.

Added audio tracks keep their absolute project starts, source trims, gain and
own fades. They are planned for one mix after the linked-clip crossfades;
adding a transition must not ripple them or apply clip envelopes a second time.
An added track referencing a video also keeps its own gain when that video's
linked clip audio is muted. Ambiguous IDs shared by media and independent audio
sources are refused instead of selecting one implicitly.
Start/end boundaries round independently to the nearest 48 kHz sample. The
source span uses the rounded source start plus the exact project sample count.
Source bounds and fade sums are checked again after rounding. Tracks extending
past the shortened timeline fail rather than being silently trimmed. Frame and
sample quantization can therefore reject a proposed transition even when its
video overlap alone fits; an explicit layer edit is required first.

A boundary can have either the saved dip or a proposed crossfade. Different
boundaries may coexist, including opposite ends of one clip. Each half-dip
reserves `ceil(duration * fps / 2)` local frames, accounting for every frame
touched by its continuous fade. The combined incoming/outgoing dip and overlap
budget must leave two clear frames. No dip duration is rewritten or applied
twice, and no unsupported compound transition is silently substituted.

## Persistence and source boundaries

The [v1 draft/storage contract](PHASE_J_STORAGE_RECOVERY_CONTRACT.md) continues
to reject photos, titles, extra audio and existing dips. It has not been
broadened to accept data it cannot describe. The new mixed-layer plan is a
separate read-only path; actual production storage needs a versioned layer
manifest, authorized commands and migration/import rules before it can consume
these plans. Text, geometry, source paths, audio descriptors and other edits
remain bound through the whole saved-document hash even when they do not
change frame ranges. The hash is an identity receipt, not an authorization
token or a replacement for current-file verification.

No original media bytes are read by this planner. Native root authorization,
physical source verification, photo decoding/normalization, exact linked-audio
source mapping, space/cancellation handling and no-overwrite publication remain
the responsibilities of the future reviewed production route.

## Checks and remaining work

Eight added tests cover mixed and still-only clocks at both frame rates,
cumulative rounding, absolute title/audio ranges, muted/silent sources,
dip/crossfade budgets, unsupported combinations, quantization collapse/overflow
of layer spans, source validation and saved-document binding after same-revision
title edits. Fixtures preserve their sentinel original and actual project rows.
Evidence is beside the checkout under `audit-output/phase-j-layers-*`.

The full local suite passed **376 native tests** (eight explicit acceptance modes
not selected) and **332 frontend tests** across 65 files. TypeScript/Vite build
passed with the existing chunk-size warning. Final lint/format results are
clean: Clippy with warnings denied, relevant Rust formatting and diff whitespace
checks passed. A final rerun passed all 27 adapter tests after the audio identity
and independent gain edge cases were added. These results do not establish
installed-app or real-footage acceptance.

Next, build a versioned mixed-layer manifest and stale-plan validation before
connecting editor commands/undo. Then connect preview/seek and native composition
with shared frame/sample clocks and verify real-footage parity, title readability,
audio listening and installed-Windows behavior. Active crossfades remain gated.
The XMP 0.56.76 handoff stays preserved. This update is local only; no installer,
installation, push, upload or remote CI is implied.

The subsequent 0.56.80 [immutable manifest groundwork](PHASE_J_LAYER_MANIFEST.md)
implements the versioned snapshot and stale-plan check in separate disposable
storage. The results above remain this document's 0.56.79 timing checkpoint.
