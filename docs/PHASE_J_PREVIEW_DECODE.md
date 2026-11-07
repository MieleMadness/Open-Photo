# Phase J real preview-input decoding — local 0.56.85

This increment connects the [test-only request coordinator](PHASE_J_PREVIEW_REQUESTS.md)
to the bundled Windows decoder. It produces actual normalized RGB input frames
for the selected video/photo clips. It does not produce a composed preview image,
decode/mix audio, draw global titles, apply dips or enable playback/exports in the
application. All new decoder code is `cfg(all(test, windows))` or inside the
Windows test-only saved-project adapter.

## Decode and ownership contract

The worker starts only through the coordinator's active slot. It revalidates the
request and reads the authoritative selected manifest, then parses its bound
native project. Source paths come from the canonical saved-project root and
saved relative asset paths, not caller-provided normalized-part paths. This
experimental adapter supports one source root; production workspace authorization
and multi-root relinking still need their reviewed native route.

Before decoding, it checks selected source sizes and saved SHA-256 bindings and
uses the existing `PinnedMedia` guard to retain verified Windows read handles.
Repeated clips using one asset pin that source once. The existing guard refuses
hard-linked/reparse files and incompatible writers. The returned decoded-input
object owns these source handles through subsequent eligibility checks and
future composition. The consumer must promptly drop obsolete inputs; they are
not a permanent preview cache. No original file is opened for writing.

Video preparation reuses the current renderer's source probe, trim mapping,
edit filters, preview canvas and SDR checks. The clip normalization filter was
extracted unchanged from `part_args` into one shared helper, used by encoded
parts and this decoder. It retains cumulative native frame allocation, FPS,
padding and normalized zero-based frame selection. Original file PTS values are
not guessed from the editor playhead.

Photo preparation reuses verified decoding, geometry checking and flattening of
transparency onto black. Its temporary PNG is created exclusively in an owned
scratch directory and pinned against a hash of the encoded bytes. Handles are
released before guarded scratch cleanup. The same edit/canvas/frame filters then
produce one RGB input frame. There is no external normalized-file receipt that
can assert its own source provenance.

The worker selects exactly the requested normalized-part frame and reads RGB24
through the existing bounded process runner. It requires exactly
`width * height * 3` bytes. One or two returned parts must match the requested clip
order and canvas, and fit the existing 8 MiB combined payload budget. Their typed
result is deliberately separate from the coordinator's opaque display-payload
test seam: raw overlap inputs are not mislabeled as a finished preview.

## Bounds, cancellation and stale work

This engineering path currently limits each selected clip to ten seconds,
saved source files to 128 MiB and saved input geometry to 8,388,608 pixels.
The shared preview canvas is at most 960 × 540. These conservative experiment
bounds do not restrict today's production renderer. Large/full-length preview
support and cache/performance policy remain future work.

Actual FFmpeg frame decoding uses a 30-second timeout, bounded stdout and the
existing Windows process containment/reaping runner. Metadata/source preparation
uses the existing helpers and their own limits; this is not a new 30-second
deadline for the entire job. Cancellation is checked through source verification,
preparation, decode and between selected inputs. A new seek after one input has
decoded prevents the remaining obsolete inputs from being returned.

Typed decoded results pass the coordinator's common completion path, which
acknowledges the exact active worker and revalidates native session/project state.
Cancelled or stale work is discarded and its source handles are released. A
typed consumer check revalidates eligibility before future composition. The
future view must still coordinate final delivery and share the application's
processing budget. This synchronous gated executor is not connected to a UI
thread, global production scheduler or actual playback loop.

## Verification and next work

Seven added bundled-engine regressions use disposable red/blue video and a
transparent PNG. They verify exact source trim/frame selection across a color
change, both overlap inputs, photo flattening, RGB bounds, retained source pins,
same-revision project edits during decode, supersession between real decoded
inputs, modified/hard-linked source refusal, duration limits and false geometry.
The sentinel original and actual generated source bytes are preserved except
for deliberate, restored fault injection in owned fixtures.

The corrected targeted run passed **71 saved-adapter tests**. Its initial failed
run is retained as `phase-j-decode-initial-targeted.log`: three assertions assumed
a 96 × 64 project while the fixture retained the default widescreen canvas and
sampled its padded border. Setting the fixture's saved canvas explicitly fixed
those assertions; the decoder continues to honor the saved project canvas.
Evidence is beside the checkout under `audit-output/phase-j-decode-*`.

The final validation run passed **420 native tests** (eight explicit acceptance
modes not selected), **332 frontend tests** across 65 files, TypeScript/Vite build
and Clippy with warnings denied. The existing chunk-size warning remains. Relevant
Rust formatting and scoped diff whitespace checks passed. Separate library/HEIC/
thumbnail changes appeared concurrently in the shared workspace and were left
untouched; repository-wide whitespace checks report their extra EOF blank lines
in `src/App.tsx` and `src/styles.css`. These run results do not sign off that
separate workstream, installed behavior or real-footage/hardware acceptance.

Next, compose these verified video/photo inputs with the planned dissolve/dip
weights, then add global title rendering and actual PCM audio-window processing.
Only after content/parity tests should the result become a display-ready preview.
Production activation still requires source/root authorization and path-identity
acceptance, migration/import and old-client behavior, reviewed audio/compositor
policies, real-footage preview/export parity and installed-Windows acceptance.

This source remains local with the 0.56.84 request-coordinator work. The last
pushed tree is still 0.56.83 in draft PR #121; its passing fast CI does not cover
these local updates. No new push, installer, installation or remote CI is included.
The preserved XMP 0.56.76 installer/source handoff is unchanged.
