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

## Version 0.56.68 batch hardening (pending verification)

The batch tag path now checks sidecar directory entries via `symlink_metadata` before choosing the create path, and treats an XMP-preservation failure as a per-item error. Tests cover a non-file sidecar while continuing the next file and a Unix dangling link. These changes do **not** close the sidecar compare-to-publish race, Windows junction/revocation or crash-recovery requirements. Review exact-head Windows CI and installed real-media evidence before release.
