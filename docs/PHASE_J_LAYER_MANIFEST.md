# Phase J immutable mixed-layer manifests — local 0.56.80

This increment saves and revalidates the [mixed-layer timing plan](PHASE_J_LAYER_TIMING.md)
against the actual native sequence reader. All code remains in the `cfg(test)`
adapter and owned disposable fixtures. No production commands, migrations,
editor controls, preview or export consume these records.

## Record and validation contract

Manifest v1, timing-policy v1 records contain the canonical root, native project
ID/revision, whole stored-document hash, exact saved project JSON, ordered
crossfade boundaries and the derived frame/sample plan. Keeping the saved JSON
binds source descriptors, rational trims, photo geometry, title text/positions,
audio metadata/gain/fades, dips and clip edits even when timing stays unchanged.
It is not proof that physical source files still match these descriptors.

Preparation reads the project and its raw stored JSON in one transaction.
Boundaries are emitted in native clip order. Save/load independently read the
current authoritative project, reconstruct its plan with the native validator
and compare the entire manifest. The saved JSON is never used as its own
authority. A changed revision, same-revision document edit, forged timing,
foreign root/project, unknown version/policy or invalid boundary fails without
overwriting records. Unknown and duplicate fields in the envelope or typed plan
are refused. JSON is limited to 4 MiB before deserialization.

The record's ID is SHA-256 of its canonical typed serialization. Whitespace and
object-key order in an otherwise identical incoming JSON do not create a new
record. Boundary array order is canonical: preparation normalizes it; externally
reordered arrays are refused during validation. Checksums provide integrity,
not authentication against an actor rewriting the database and its contents.

## Immutable disposable storage

Records live in a separate `phase_j_layer_manifests_test` table, with root,
project and content ID as its primary key and an explicit record format version.
Initialization is atomic/idempotent and checks the exact private schema SQL.
Any schema variation is refused and retained for review rather than silently
repaired. This literal-schema rule is intentionally strict for disposable
fixtures; production migration/version negotiation is still a separate gate.
The application's database version and native project rows are not changed.

Saves use an immediate transaction. Different valid proposals coexist as
separate immutable records; there is no current-head pointer or automatic
replacement. Repeating a committed save returns its existing content ID without
writing. If that ID's existing row is corrupt or from an unsupported format,
the save fails and preserves it. A lost reply can be retried without duplicates,
provided the base project still matches. A changed base project invalidates
even a previously completed retry; no implicit rebase or activation occurs.

Loads verify the scoped record ID/checksum/version and recompute the complete
plan. Stale and corrupt records remain stored; they do not become empty drafts.
There is no pruning, mutable editor revision, mixed-layer undo stack, recovery
selection UI or rebasing in this increment. Those require explicit contracts.
The earlier video-only v1 draft/history/recovery-copy store remains separate and
continues to reject mixed layers.

## Checks and next work

Eight new regressions cover mixed-project cold reopen, immutable/canonical
retries, separate proposals across connections, forged/unknown/duplicate fields,
same-revision metadata changes, future/corrupt/schema records, rollback and three
actual child-process exits at insert/pre-commit/post-commit boundaries. Owned
fixtures preserve sentinel originals and the native project JSON. Local evidence
is beside the checkout under `audit-output/phase-j-manifest-*`. SQLite crash
recovery tests do not establish hardware power-loss or installed-app acceptance.

The full local suite passed **384 native tests** (eight explicit acceptance
modes not selected), including all 35 saved-adapter tests. **332 frontend tests**
passed across 65 files; TypeScript/Vite build passed with the existing chunk-size
warning. Clippy with warnings denied, relevant Rust formatting and diff
whitespace checks passed. The initial targeted log retains a test assertion
failure caused by a noncanonical folder key in a direct SQL check; it was
corrected before the passing full-suite run. That correction did not change the
storage contract or production behavior.

Next, add a gated editor session with explicit current-manifest selection,
expected-revision edits and undo/redo, reusing this revalidation before state
changes. Production integration still needs authorized source/root validation,
storage migration/import, retention/recovery policy and a version older clients
reject. Shared preview/seek and native composition then need real-footage,
title-readability, audio-listening and installed-Windows acceptance.

The XMP 0.56.76 handoff is preserved. No installer, installation, push, upload or
remote CI is included in this local backend-only update.

The subsequent 0.56.81 [editor-session groundwork](PHASE_J_EDITOR_SESSION.md)
adds a gated in-memory current selection and undo/redo over these immutable
records. It does not add a durable current-head pointer or migrate production
data. The results above remain this document's 0.56.80 manifest checkpoint.
