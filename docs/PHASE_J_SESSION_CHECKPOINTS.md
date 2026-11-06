# Phase J saved session checkpoints — local 0.56.82

This increment gives the [test-only editor session](PHASE_J_EDITOR_SESSION.md)
explicit durable checkpoints, latest-checkpoint resume and reviewed historical
recovery into a separate session. All code remains inside the `cfg(test)` native
adapter. No production commands, editor controls, preview/export routes or
application databases use it.

## Save and resume contract

Checkpoint v1 contains the whole session: canonical root/project, session ID and
revision, selected immutable manifest, undo/redo stacks and optional recovery
provenance. It also binds the authoritative saved-project revision and complete
stored-document hash. Checkpoint payloads are limited to 64 KiB before parsing;
SQLite reads also bound materialized payloads. Unknown fields, unsupported
versions, invalid identities/revisions and invalid history shapes fail.

Saving and resuming independently revalidate the selected manifest **and every
undo/redo reference** against the current native project and timing. The combined
history remains bounded to 20 changes. A corrupt inactive snapshot blocks resume
rather than being silently dropped. Same-revision title/source/metadata edits
invalidate checkpoints through the complete saved-document binding. Root and
project scoping remains separate from production authorization.

Checkpoints are append-only rows keyed by root/project/session/revision, with a
SHA-256 payload checksum. The schema is a separate literal-checked disposable
table; unknown schema variations are retained and refused. The application's
database version, saved project JSON and previous video-only recovery tables are
not migrated or changed. Checksums detect corruption; they are not authentication
against an actor rewriting database contents and hashes together.

A save supplies the exact latest checkpoint receipt it expects to extend.
Independent resumed writers cannot silently replace one another's progress:
conflicting contents at the same revision or a changed latest receipt fail.
Multiple edits between checkpoints may produce a revision gap. Retrying an
already committed identical checkpoint returns its original receipt, including
after a later checkpoint exists; this acknowledges that save and does not select
an older head. A normal resume explicitly requires the latest receipt visible in
its read transaction. A concurrent later save can still make that session stale;
its next checkpoint detects the conflict.

## Explicit recovery and failure boundaries

Historical recovery first verifies a chosen immutable checkpoint and creates a
review selection with a stable new session ID. Applying that selection creates a
separate session at revision one, preserving the selected manifest and history
and recording the source checkpoint receipt. It never replaces the original
session's latest checkpoint. Repeating the same recovery selection reuses the
same verified copy; a lost reply does not create duplicate sessions.

Recovery still requires the original project and every referenced manifest to
match. It is not a rebase after external edits, an offline source bypass, or a
way to ignore corrupt history. Invalid or unverifiable rows remain stored for
inspection. Newer checkpoints on the original session do not block deliberate
recovery of a verified older checkpoint into a separate copy.

Edits and checkpoint saves are **separate explicit operations** in this gated
API. An edit persists its immutable plan before a session checkpoint is saved.
If the process exits during a checkpoint, only the committed checkpoint is
resumed; newer uncheckpointed plans remain stored. There is no autosave promise
or durable current-state change on every edit. Choosing when production should
checkpoint is still part of future editor integration.

Checkpoint and recovery writes use immediate SQLite transactions. Errors before
commit roll back new rows; errors after commit leave a verifiable record that
can be acknowledged on retry. Saving a checkpoint does not mutate the working
in-memory session. No history, checkpoint or manifest pruning is performed.

## Checks and next work

Eight new tests cover full selection/revision/undo/redo cold reopens, independent
connection conflicts, historical recovery copies and lost replies, checkpoint
and recovery rollback, corrupt/future/oversized/unknown records, changed schemas,
foreign roots/projects, stale native documents and corrupt inactive history.
Three actual child-process exits exercise checkpoint insert/pre-commit/post-commit
boundaries while retaining the separately committed edit manifest. They prove
local SQLite process-crash behavior, not hardware power-loss durability or
installed-app acceptance. Logs are beside the checkout under
`audit-output/phase-j-checkpoint-*`.

The targeted run passed all **50 saved-adapter tests**, including the eight new
checkpoint regressions and the existing manifest/session recovery tests.

The full local suite passed **399 native tests** (eight explicit acceptance modes
not selected) and **332 frontend tests** across 65 files. TypeScript/Vite build
passed with the existing chunk-size warning. Clippy with warnings denied,
relevant Rust formatting and diff whitespace checks passed. No installed-app,
real-footage or hardware power-loss acceptance is claimed by these results.

Next, map a resumed mixed-layer plan onto the shared preview/seek frame and
sample clock. Keep production activation gated on authorized physical source
verification, migration/import and old-client behavior, compositor/audio policy
review, real-footage preview/export parity and installed-Windows acceptance.
Retention and checkpoint scheduling also need production decisions.

The XMP 0.56.76 handoff is preserved. This local update includes no installer,
installation, push, upload or remote CI.

The subsequent 0.56.83 [session seek groundwork](PHASE_J_SESSION_SEEK.md) maps
resumed selections to verified frame/sample descriptions without decoding or
changing the checkpoint contract. Results above remain this document's 0.56.82
saved-session milestone.
