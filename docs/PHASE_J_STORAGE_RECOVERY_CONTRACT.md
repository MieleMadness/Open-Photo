# Phase J storage and recovery contract — local 0.56.78

This increment extends the [saved-project adapter](PHASE_J_SAVED_PROJECT_BINDING.md)
with executable storage/recovery rules. All implementation remains inside the
`cfg(test)` adapter; no application command, production migration, editor control,
preview or export uses it. Tests open only owned disposable SQLite fixtures.
The preserved XMP 0.56.76 installer/source handoff is unchanged.

## Version ownership and compatibility

The prototype has a component-owned v1 marker and separate `_test` tables for the
current draft, history and recovery copies. Initialization is atomic and
idempotent. The existing application database's `PRAGMA user_version`, actual
sequence tables, schema-v3 project JSON and base-project history policy are not
changed. Table columns, types, required fields and primary keys are checked.
Unknown versions, partial schemas and the earlier unversioned disposable
prototype are refused with their rows preserved. There is no silent upgrade or
backfill of history that lacks project provenance.

This is a proposed contract, **not a production migration**. Before enabling
active crossfades, choose a new project format that older clients reject rather
than silently playing hard cuts. The current native reader rejects schema 4;
that test establishes the existing compatibility boundary, not a completed
schema-4 implementation. A current client can still save an ordinary schema-v3
project; its revision/document change makes old detached drafts, history and
copies stale. Stored data is retained without automatic rebasing.

## Integrity, history and recovery

Draft/history/copy payloads carry SHA-256 checksums plus the authoritative
project revision and whole stored-document hash. Existing strict snapshot,
source-descriptor and frame validators still apply. Checksums detect accidental
payload changes; they are not authentication against an actor rewriting a
database and its checksums. Root/project scoping also does not replace the
production Windows authorization and source-verification pipeline.

History and recovery copies are append-only in this gated experiment. Reads use
25-entry descending pages with lookahead; there is no automatic pruning. A bad
payload is reported on its individual history entry while valid entries remain
available. Invalid database representations or unreadable storage fail the
request explicitly, rather than returning empty history. Production disk budgets,
retention controls and reviewed pruning remain open design gates.

Recovery is explicit selection followed by creation of a **separate draft copy**.
Selection reads a verified history row and captures a fingerprint of the current
head. Creation rechecks the native project, selected history and head in an
immediate transaction. It never updates the head, history or actual project.
A corrupt head payload can therefore be preserved while a verified historical
draft is copied; malformed database field representations still fail safely.
Copies remain detached and are not automatically activated or rendered.

Each selected request gets a stable copy ID and complete operation fingerprint.
Repeating an already committed request returns the same verified copy, including
after a lost reply and a subsequent head change. A different operation using the
same ID fails. A base-project change still invalidates the request and copy;
there is no automatic rebase. A future public API must supply authorized,
server-owned selection state rather than treating these internal values as an
authorization token.

## Validation and limits

Nine added tests cover component versioning, idempotent initialization,
old-client/stale binding, future/unversioned/partial/keyless schemas, explicit
scoped copies, corruption preservation, selection races, retry identity,
transaction rollback, paged retained history and abrupt child-process exits.
The child helper returns immediately during ordinary runs; its parent exercises
seven actual exits across draft/history and recovery-copy commit boundaries.

WAL-backed fixtures are reopened twice after injected errors and process exits.
Uncommitted changes roll back; committed copies survive and retries create no
duplicates. This tests local SQLite process-crash recovery, **not hardware
power-loss durability**, installed-app behavior or real-footage rendering.

The full native suite passed **368 tests**; the frontend suite passed **332 tests**
across 65 files, and TypeScript/Vite build passed with the existing chunk-size
warning. Logs are beside the checkout under `audit-output/phase-j-storage-*`.
The final targeted rerun passed all 19 adapter tests after adding the keyless
schema case. Clippy with warnings denied, relevant Rust formatting and diff
whitespace checks passed. Eight existing explicit acceptance modes are not
selected by this increment. No installer, installation,
upload, push or remote CI is part of this backend-only update.

## Next work

The subsequent local 0.56.79 [layer timing groundwork](PHASE_J_LAYER_TIMING.md)
implements a separate read-only planner for the timing proposals below; the v1
persisted draft format still rejects mixed layers. Results in this document
remain the 0.56.78 storage checkpoint.

Define the overlapping frame clock for photos, title ranges, audio tracks and
existing dips, then extend the native binding tests. Keep unsupported projects
blocked in this adapter until those semantics are implemented. Production
storage migration/import, authorized editor commands and undo, synchronized
preview/export, real-footage parity and installed-Windows acceptance remain
separate gates before enabling crossfades.
