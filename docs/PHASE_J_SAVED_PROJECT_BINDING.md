# Phase J — saved-project binding groundwork (0.56.77)

This increment connects the experimental crossfade draft contracts to the actual
native sequence reader, schema and frame-allocation code, using disposable
file-backed SQLite fixtures. The entire adapter is behind `cfg(test)` inside
`video_sequence::phase_j_sequence_binding`. It adds no Tauri command, production
migration, active transition control, preview route or export route.

## Native context and optimistic concurrency

The adapter reads the root-scoped `video_sequences` row through the real
`video_sequence::read` path. It derives ordered clip frame allocations using
`video_sequence_render::frame_allocation`, and source descriptors from the saved
assets and exact rational trim marks. Expected sources never come from the
untrusted draft envelope. Existing strict native snapshot/record validators
check the resulting context and each incoming draft.

The context binds both the saved revision and SHA-256 of the complete stored
project JSON. This catches edits, output changes and even stored-byte changes
that leave revision, clip frames and source descriptors unchanged. The hash is
an optimistic concurrency token, not an authentication capability or a live-media
integrity check. Revision values must fit the frontend's safe integer range.

Each draft commit uses an immediate transaction that reloads the authoritative
project before checking its document hash, project revision and expected draft
revision. Drafts/history are written only to separate `_test` tables. Stale
writers are refused before idempotence checks; a current identical write does
not increment revision or add history. History and current draft changes commit
or roll back together. The prototype retains its fixture history; production
retention, history selection and explicit rebase/copy policies remain to be designed.

Loading does not turn malformed, stale or orphaned data into an empty draft.
Invalid JSON, mismatched source bindings, changed/deleted projects and corrupt
revisions produce errors while retaining stored drafts. This does not yet provide
a production recovery UI or a tested process-crash/power-loss guarantee.

## Compatibility boundary

This binding subset requires at least two referenced video clips and the existing
30/60-fps/frame/record-size constraints. It rejects dips, titles, audio tracks /
independent audio declarations and referenced photos rather than silently dropping
or retiming them. Otherwise valid active projects can therefore be ineligible for
this experimental subset. Native v2 parsing is reused and does not rewrite its
stored legacy JSON while creating a detached draft.

Ordinary hard cuts, dip-to-black, photos, titles and audio continue using their
existing production code. The adapter does not open the application database or
read original media bytes; fixtures construct synthetic saved descriptors and
check the original sentinel after each test. Current on-disk source verification,
Windows identity/root authorization, normalization and no-overwrite publishing
must still be supplied by the separately reviewed native pipeline before any
future production route can use a validated draft.

## Validation

- **359 native tests passed**, including all ten new adapter regressions; eight
  existing explicit acceptance modes were ignored and not selected in this update.
- **332 frontend tests passed** across 65 files. TypeScript/Vite build passed;
  the existing bundle-size warning remains.
- Clippy with warnings denied, full relevant Rust formatting and diff whitespace
  checks passed. No browser/installed GUI claim is made for this backend-only work.

Evidence is stored beside the checkout in `audit-output/phase-j-binding-*`.
Ten new regressions exercise the real saved reader/schema, cumulative frame
rounding, detached save/reopen, independent connections, stale/no-op writers,
same-revision project edits, forged snapshots, wrong roots, deleted projects,
transaction rollback at three boundaries, unsupported active content, legacy v2
compatibility and safe revision/ID checks. The fixtures use WAL-backed temporary
databases with full synchronization; cold reopening is distinct from abruptly
killing a process or losing hardware power.

## Next increments and gates

The subsequent local 0.56.78 [storage/recovery contract](PHASE_J_STORAGE_RECOVERY_CONTRACT.md)
replaces this disposable prototype's unversioned storage with a gated v1 marker,
provenance/checksums and explicit recovery copies. It does not migrate production
data or enable active crossfades. The results above remain the 0.56.77 checkpoint.

1. Review a versioned production draft/storage contract: owner/root authorization,
   migration and old-client behavior, history retention, recovery and explicit
   rebase/copy choices. Do not put this prototype's `_test` schema into user data.
2. Define mixed-media, title/audio timing and existing-dip coexistence on the
   overlapping frame clock; extend the native binding contract accordingly.
3. Integrate feature-gated editor commands/undo, preview seeking and native export
   only after source, timing and recovery contracts have been reviewed.
4. Retain real-footage preview/export parity and installed-Windows acceptance
   before enabling production crossfades. XMP's installed/durability gates remain
   separate; its preserved 0.56.76 handoff is unchanged.

This update stays local. No installer is rebuilt, installed or uploaded, and no
remote CI or release is claimed. Earlier dated PR states in VIDEO_ROADMAP are
historical; this document describes the current local groundwork only.
