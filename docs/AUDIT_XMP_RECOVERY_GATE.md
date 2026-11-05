# XMP sidecar recovery and external-editor conflict gate (F07/R01)

Status: **test design only; no production XMP behavior changed**. Based on audit integration 0.56.67 (PR #86, head `c34255b`). This checklist must be converted to executable native fault-injection tests before production changes merge. Do not claim an XMP recovery defect has been reproduced until a fixture fails.

## Safety invariants

1. Originals remain byte-identical. All test files use disposable synthetic media and sidecars.
2. An XMP write must never silently overwrite an external editor's newer changes. Revalidate the sidecar identity/content immediately before publication; if it differs from the previewed basis, abort and ask the user to re-preview/reconcile.
3. Commit must use a unique same-directory temporary file, flush contents and metadata as appropriate, and publish without exposing a partially written final file. Record the platform-specific durability guarantees rather than assuming an atomic rename implies power-loss durability.
4. Every recoverable previous sidecar is preserved or recoverable after interrupted writes; no auto-deletion of ambiguous backups or temporary files.
5. Recovery distinguishes clean, pending write, succeeded-but-unacknowledged write, conflicting external edit, corrupt temp, incomplete backup and permission/IO failure. Never silently pick one candidate when multiple plausible versions exist.
6. Repeated recovery is idempotent, including restart after a failed recovery attempt; path traversal, junction/reparse aliases, hard links and registered-root revocation fail closed.
7. UI status must reflect persisted disk state, not an optimistic in-memory preview; retries must be deliberate, and no background job may hide an unresolved conflict.

## Native test matrix

| Case | Injection / reproduction | Required assertion |
| --- | --- | --- |
| Initial write | Synthetic source, no existing sidecar | Sidecar validates; original hash unchanged; no orphan temp |
| Existing sidecar | Valid old sidecar with metadata outside Open-Photo's managed fields | Preservation/merge policy is explicit; backup remains restorable |
| Simultaneous external edit | External process changes sidecar after Open-Photo preview, before commit | Conflict reported; external bytes unmodified; no silent overwrite |
| Same-length external edit | Mutate sidecar content preserving byte length and modification time where supported | Conflict detected by content identity, not size/rounded timestamp only |
| Rename race | Replace sidecar between preflight and publish | Fail closed or demonstrate handle-based protection; no replacement lost |
| Interrupted temp write | Inject partial write/flush error | Final sidecar unchanged; corrupt temp quarantined or safely identified |
| Interrupted publish | Stop after temp durability, before final promotion | Restart identifies pending state; no destructive automatic promotion |
| Interrupted acknowledgement | Stop after durable publication, before clearing recovery marker | Restart recognizes committed bytes without duplicating revision |
| Backup failure | Deny backup creation or force short write | No final mutation; precise user-facing error |
| Read-only / full volume | Deny write, rename or final flush | Previous sidecar intact; no successful-save notification |
| Corrupt existing XMP | Truncated XML and unsupported namespace | Existing bytes preserved; no destructive repair |
| Two writers | Both preview same base; writer A commits first | Writer B rejects stale preview; no last-writer-wins data loss |
| Restart and retry | Crash each write stage; restart twice | Recovery state stable and idempotent |
| Unsafe path | Symlink/junction outside registered root; revoke folder while operation pending | No writes outside approved root |
| Metadata compatibility | Preserve third-party XMP properties and encoding variants | Round-trip non-owned fields or clearly block unsupported cases |

## Required implementation order

1. Inventory actual XMP write, backup, rename, recovery and conflict call sites on the latest merged desktop source. Document current atomicity guarantees and error surfaces. Do not import an older audited implementation over newer code.
2. Add tests that inject failures at every durable transition: old file read, backup creation/flush, temp creation/write/flush, identity recheck, publish, parent directory sync when supported, acknowledgement and cleanup.
3. Add explicit persisted recovery states and strict conflict detection *only where tests demonstrate a missing guarantee*. Favor additive migration with verified rollback; never rewrite user sidecars merely to upgrade an internal state format.
4. Expose an accessible conflict UI showing: **Review external changes**, **Keep external version**, and **Cancel**. Any eventual overwrite path requires a newly generated preview and explicit user confirmation; no implicit force-save.
5. Run native tests + TypeScript/frontend checks, Clippy, packaging, installed Windows real-sidecar editor-conflict trials, and source-hash checks on the **exact PR head**. Retain the old implementation until CI and manual safety evidence are recorded.

## Release boundary

This document does not close F07/R01. It does not authorize a release, migration, recovery cleanup or a production XMP write change. Track implementation and evidence in a subsequent focused PR. Keep crossfade production routing, vault and Immich expansion behind the existing stability gates.

## Native regression baseline (follow-up)

The test-only `src-tauri/src/xmp_recovery_tests.rs` suite exercises the existing `replace_managed_sidecar` path with disposable synthetic sidecars: successful replacement and cleanup, same-length changed content detected before replacement, preexisting backup/temporary recovery files refusing writes, and malformed UTF-8 preserving bytes. It is registered under `#[cfg(test)]` in `lib.rs`.

**Not yet covered:** deterministic replacement *between* final compare and atomic publish; power loss/rename injection; parent-directory durability; true concurrent external-process file replacement; junction/root revocation; installed Windows external-editor trials. These remain F07/R01 release blockers. Passing this regression suite must never be described as closing the full gate. No production XMP write behavior is changed by these tests.

## Additional metadata-preservation regression (test-only)

The isolated `src-tauri/src/xmp_batch_preservation_tests.rs` module exercises a mixed bulk-tag request: merge a managed Open-Photo sidecar while retaining unknown third-party XML, leave foreign-created sidecars byte-identical, keep both source photos byte-identical, and confirm a repeated operation is idempotent. A malformed XMP fixture must be skipped rather than repaired or rewritten. This is **not** an external-editor concurrency or crash-recovery test and does not close Issue #90.

## 0.56.68 XMP batch safety follow-up (pending full Windows verification)

The source-side batch path uses `symlink_metadata` instead of `exists()` before creating sidecars so dangling symlinks are not treated as missing, and a preservation failure is reported for its own photo without stopping the full batch. Native fixture tests check directory entries and (on Unix) a dangling link. These fixes do not resolve Issue #90's final compare-to-publish race, Windows reparse/root revocation and crash recovery; review those independently before shipping.

## Stale-save and recovery-artifact native fixtures (test-only)

The `src-tauri/src/xmp_conflict_state_tests.rs` suite checks sequential stale-save rejection: once a managed sidecar is updated, an operation with its old expected contents must fail, and a refreshed snapshot can write. Existing recovery-backup artifacts must block repeated writes; a Unix dangling backup symlink must not be followed or reused. These tests are synthetic and sequential. They do **not** establish atomic filesystem compare-and-swap, prove a crash-safe recovery state machine, or close the external editor's compare-to-publish race. Issue #90 remains open.

## Deterministic publish-error test seam (test-only, Issue #90)

The native `replace_managed_sidecar_with_publish` helper keeps the production code path bound to the existing `atomic_replace` implementation. Test-only callbacks simulate a publication error after staging and backup are complete. The regression fixtures assert that (1) the old sidecar stays intact when publication fails, (2) the original backup remains for inspection, (3) staging is removed, (4) a subsequent save refuses the ambiguous recovery artifact, and (5) an external writer's changed sidecar bytes are not destroyed when the publish callback explicitly returns an error.

**Limit:** This deliberately simulates **failed** publication. It does not demonstrate safety if an external writer modifies the destination and the OS replace succeeds, nor does it cover a crash, interrupted backup flush, Windows handle identity checks, parent directory durability, or installed external-editor concurrency. These are still unresolved under [Issue #90](https://github.com/MieleMadness/Open-Photo/issues/90).
