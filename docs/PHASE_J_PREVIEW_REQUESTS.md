# Phase J bounded preview requests — local 0.56.84

This increment builds a test-only request coordinator around the verified
[session seek map](PHASE_J_SESSION_SEEK.md). It manages admission, coalescing,
cooperative cancellation and result eligibility. It does not spawn a real decoder,
open source media, invoke FFmpeg, decode images, mix audio or connect production
preview/export. Worker results in these tests are opaque synthetic byte payloads.

## Work ownership and bounds

Each coordinator owns one active worker slot and one pending request. A newer
seek replaces the pending request and signals cancellation to the active one.
The active slot remains occupied until that exact worker acknowledges completion,
including cancelled/error completion. Cancellation alone never starts a second
worker while the first might still be using resources.

Each request has a native coordinator ID, a monotonically increasing generation,
canonical root/project scope, exact session receipt and verified seek description.
Equivalent requests for the same quantized frame and complete native/session
binding share one pending or active ticket. Invalid new seeks invalidate obsolete
work rather than letting it return under the new playhead position. Generation
exhaustion closes the coordinator and cancels its outstanding work.

Admission and completion are serialized under a mutex while SQLite metadata is
verified. Workers can read an atomic cancellation token without taking that
mutex. This is a synchronous engineering contract, not a claim that SQLite work
belongs on the future UI thread. Scheduling, decode budgets and timeouts still
need integration with the existing native processing services.

Only the owner of the active ticket can release its slot. An old or foreign
completion cannot release a newer worker; an old cancellation cannot erase a
newer pending seek. Closing stops admission, clears pending work and signals the
active worker. Dropping the coordinator signals owned tokens even after a poisoned
mutex. It does not claim to join threads or reap media processes; the future
executor must acknowledge cancellation and clean up its own resources.

## Validation and handoff

The selected manifest and seek description are independently verified at admission,
again before pending work becomes active, and again when its active completion is
eligible for delivery. Changes to session selection, native project contents or
source descriptors block delivery even without a new seek. Physical current-file
verification remains a separate decode-pipeline requirement.

Superseded results and errors are discarded. Current errors are returned and
release the worker slot. Successful payloads must be nonempty and no larger than
8 MiB. The coordinator stores no completed-frame cache; the consumer owns any
accepted payload. This checks retained payload eligibility, not the memory a
decoder may allocate before returning. Image format/dimensions, actual PCM and
decoded-content validation remain future executor responsibilities.

Accepted results carry coordinator/generation, canonical folder/project, session
receipt, project revision/hash and seek description. A final consumer check can
reject them after completion if a later seek, session change, close or native
project edit has occurred. A future view must call that check immediately before
handoff and coordinate its own event-loop updates. These synchronous checks do
not implement UI message delivery or make a database transaction atomic with a
later physical decode or screen update.

## Local verification and next work

Seven regressions cover a 500-seek burst, equal-frame coalescing, invalid/stale
requests, a channel-coordinated simulated worker, pending/start/completion
revalidation, final folder/project handoff checks, late errors, result size bounds,
foreign/duplicate completion, cancellation/close/drop, poisoned-state cleanup and
generation exhaustion. The deliberate worker-owner panic is caught by its test;
its diagnostic is expected evidence rather than a failing test. The initial
targeted run passed all 64 saved-adapter tests. Logs are beside the checkout under
`audit-output/phase-j-preview-*`.

The final full suite passed **413 native tests** (eight explicit acceptance modes
not selected), including the added final-handoff scope/document checks. **332
frontend tests** passed across 65 files; TypeScript/Vite build passed with the
existing chunk-size warning. Clippy with warnings denied, relevant Rust formatting
and diff whitespace checks passed. These checks do not establish real decoding,
playback, installed-app or hardware power-loss acceptance.

Next, connect a gated decode executor using verified sources and bounded normalized
parts, then validate actual decoded frame/audio payloads and composition before
view integration. Production migration/import, source/root authorization, reviewed
audio/title policies, real-footage preview/export parity and installed-Windows
acceptance remain required before enabling crossfades.

This increment remains local on `feature/phase-j-preview-requests-0.56.84`.
The previously pushed 0.56.83 integration is [draft PR #121](https://github.com/MieleMadness/Open-Photo/pull/121),
commit `215d72b00115c9855901ffd6ae6b2857b60e9075`; its fast
[CI #1434](https://github.com/MieleMadness/Open-Photo/actions/runs/42296561870) passed,
with full installer verification skipped. That run does not cover this new local
source. No new push, remote CI, installer or installation is included here. The
XMP 0.56.76 installer/source handoff remains preserved.

The subsequent 0.56.85 [real decoder groundwork](PHASE_J_PREVIEW_DECODE.md)
reuses this lifecycle for typed RGB input frames from verified sources. It does
not turn opaque test payloads into a production preview or change the one-active/
one-pending bound. Results above remain this document's 0.56.84 milestone.
