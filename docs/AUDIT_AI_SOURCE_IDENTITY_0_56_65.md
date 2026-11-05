# Audit F02 — Shared AI source identity (0.56.65)

Originally based on desktop 86367d3 (0.56.63); now reconciled with merged PR #82 / desktop c67dc56e (0.56.64). Exact-head verification is required before integration. No release publication.

## Contract

A source version contains byte length, seconds plus nanoseconds, and a streamed full-content SHA-256 digest. Manual Sync persists that version beside the normal display timestamp. Queue reconciliation, counters and completion fences include it. Semantic, OCR and face cache currency compares it; result rows and the version ledger publish in one SQLite transaction after checking the source again. Video staging has its own source-version ledger and discards incompatible partial work before resuming. Content providers join their result ledger to the saved media version before scan/ranking limits.

## Migration and recovery

Existing media gains a nullable version column. Existing AI records without a version ledger are hidden and cannot satisfy cache currency. Existing queue successes with no version become pending. Choose Sync Now to upgrade the saved snapshot and let indexing rebuild. Reviewed People identities are retained; no source files are changed. Incomplete scans retain the previous durable snapshot.

## Cost and boundaries

Full hashing reads source content on Sync and during AI verification. Scratch space is bounded at 128 KiB per hash, but large-library I/O/latency is not measured. Offline saved-library restore does not hash source files. No claim is made to solve external process path-substitution races or to detect unsynced changes in offline search; search freshness is relative to the saved Library snapshot.

## Checks

Local after baseline reconciliation: 64 frontend files / 295 tests passed, TypeScript passed, production build passed, matching Rust formatter passed, and 15 native SELECT queries prepared against migrated schemas using SQLite. Native regressions added for preserved-timestamp content/size replacements, vector/text rollback and replacement, legacy ledgers, stale queue completion, and search visibility. Compilation, Clippy, native and real-model tests require exact-head Windows CI. Full installer acceptance and installed hands-on QA remain separate gates.

Next audit work: settings startup recovery (F06), XMP recovery states/conflict tests (F07/R01), and the remaining native authorization and performance findings. Reassess each against current source before changing it.
