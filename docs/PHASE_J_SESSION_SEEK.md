# Phase J resumed-session seek mapping — local 0.56.83

This increment maps the selected [mixed-layer manifest](PHASE_J_LAYER_MANIFEST.md)
of a working or [resumed session](PHASE_J_SESSION_CHECKPOINTS.md) into a native
frame/sample seek description. The implementation remains inside the `cfg(test)`
adapter. It does not decode media, invoke FFmpeg, play/mix audio, register a
command or connect production preview/export.

## Mapping contract

A seek requires the exact session ID, revision and selected-manifest receipt.
Inside one read transaction, it reloads and revalidates that manifest against
the actual saved project and native timing plan. It never uses a stale cached
plan as its own authority. Results include that session receipt, saved-project
revision and whole document hash, so future request/delivery code can check
whether a result still belongs to the active session/project. Delivery-time
stale-response rejection is not implemented by this synchronous mapper.

Working selections may include edits not yet checkpointed. Seeking does not
automatically switch to a newer checkpoint from another working branch or write
session history; conflicting checkpoint saves remain governed by their own
expected-receipt contract.

Frame positions are integers on the shared 30/60 fps clock. Approximate seconds
use the earlier TypeScript seek policy: `floor(seconds * fps + 1e-9)`, with
finite/nonnegative validation and an exclusive project end checked before and
after quantization. Seeking the end never duplicates the final frame.

Video descriptions name one or two active clip/asset IDs, photo identity and
the zero-based normalized-part frame. Crossfade video weights are exact integer
fractions sampled at frame centers: incoming `(2 * local + 1)/(2 * overlap)`;
outgoing is its complement. A single active clip has unit weight. Photos use
their normalized still-part frames without invented source video timestamps.
Dip-in/out values describe reserved local video-frame budgets from the timing
plan; they are not opacity multipliers or a replacement for the saved continuous
dip duration. Exact dip rendering remains a compositor integration gate.

Titles are returned as project-global IDs once in their half-open quantized
range, even when two clips are active. Text and styling are still obtained from
the same verified saved manifest/project. Their proposed post-transition
compositor placement remains the policy described in the layer timing document.

Each frame also covers an exact half-open 48 kHz sample window: 1,600 samples
at 30 fps or 800 at 60 fps. Linked clip audio slices share that clock, identify
their normalized-part sample start, and carry saved gain plus linear crossfade
envelopes. Missing, muted, zero-gain or photo-linked audio stays explicitly
silent. Dips never add an audio fade.

Added-track slices intersect their own sample range with the requested window.
A start or end partway through a video frame is retained exactly; it is not
rounded again to the frame boundary. Their source sample starts use the saved
48 kHz trim mapping, and gain/fades remain independent of clip crossfades.
The result distinguishes normalized-clip offsets from added-source offsets.
Neither is a guessed original file PTS. Physical stream timestamps, resampling,
trim padding and source verification still belong to the native decode pipeline.

Linear audio gains use sample-center fractions on their explicit envelope
ranges, multiplied by saved gain. This follows the native graph's preliminary
linear-audio direction. The older TypeScript sampling experiment proposes
equal-power audio; that proposal is not silently treated as parity. No existing
TypeScript or active renderer behavior is changed here. Actual decoded PCM,
FFmpeg ramp endpoints, loudness/clipping and listening acceptance still need
comparison before choosing a production policy.

## Verification and remaining work

Seven new regressions cover overlap endpoints and title ranges, linear audio
envelopes and mute/gain, subframe added-audio slices, photos/dips, complete
timeline coverage at both frame rates, time quantization/exclusive end and
receipt/project/corruption checks after checkpoint resume. Seek operations do
not change session state, manifests, checkpoints or original media. Local logs
are beside the checkout under `audit-output/phase-j-seek-*`.

The targeted run passed all **57 saved-adapter tests**, including the seven new
seek regressions and the existing manifest/session/checkpoint recovery tests.

The full local suite passed **406 native tests** (eight explicit acceptance modes
not selected), plus **332 frontend tests** across 65 files. TypeScript/Vite build
passed with the existing chunk-size warning. Clippy with warnings denied,
relevant Rust formatting and diff whitespace checks passed. These results do
not establish actual mixed-layer playback, audio parity or installed acceptance.

Next, build bounded preview work requests from these descriptions, with
cancellation and stale-result rejection at delivery. Then integrate decoding,
normalization and composition behind the existing gates. Production enablement
still requires authorized physical source verification, storage migration/import
and old-client rejection, reviewed compositor/audio policies, real-footage
preview/export parity and installed-Windows acceptance.

The XMP 0.56.76 handoff is preserved. No installer, installation, push, upload or
remote CI is included in this local backend-only increment.

The subsequent 0.56.84 [preview request groundwork](PHASE_J_PREVIEW_REQUESTS.md)
adds bounded scheduling metadata, cooperative cancellation and start/completion/
consumer eligibility checks around this seek map. It does not connect a decoder
or alter the seek timing policy. Results above remain the 0.56.83 milestone.
