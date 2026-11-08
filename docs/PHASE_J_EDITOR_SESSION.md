# Phase J gated editor sessions — local 0.56.81

This increment adds a native, test-only editor session over the
[immutable mixed-layer manifests](PHASE_J_LAYER_MANIFEST.md). It supports explicit
manifest selection, adding/changing/removing one adjacent crossfade, and
undo/redo. It is not registered as an application command and has no production
UI, preview or export connection. All tests use owned disposable databases.

## Revision and edit contract

Opening a session revalidates a saved manifest against the authoritative native
project and timing. The session has a new native session ID, a revision starting
at one, the current manifest ID, and in-memory undo/redo stacks. This session
revision is separate from the underlying saved project's revision.

Each action requires the exact session ID, session revision and current
manifest ID last returned to its caller. An obsolete or different-session
receipt fails before changing anything. This is a consistency check, not an
authorization credential. Independent sessions can create separate immutable
proposals; there is no shared mutable project head or global session lock.

An edit loads and revalidates the current manifest inside an immediate SQLite
transaction. Crossfade edits must name an adjacent pair from the actual native
project; removal also checks adjacency. The next plan is rebuilt from that
project, preserving the other proposed boundaries. Existing dip conflicts,
frame budgets and title/audio overruns remain errors. No saved project JSON,
title, audio layer or source descriptor is edited through this session API.

Selection, undo and redo revalidate the target manifest as well as the current
one. A stale native project or corrupt target fails without consuming history.
If the current manifest is corrupt, a healthy target cannot bypass that check;
explicitly opening a new session from a verified snapshot remains possible.
Nothing is rebased implicitly.

## Undo, failure and lifecycle behavior

Successful changes advance the session revision. A new edit or selection pushes
the former current ID onto undo and clears redo. Undo/redo move between verified
snapshot IDs, incrementing the session revision rather than restoring an old
receipt. Repeating the current edit or selecting the current manifest is a
no-op: revision, undo, redo and immutable record count stay unchanged.

The undo window retains at most 20 changes. Dropping an older undo reference
does not delete its immutable snapshot. Production record retention/pruning
remains a separate design gate.

Session state is installed only after SQLite commits the new immutable record.
Handled errors before commit roll back new records and retain the prior
selection/history. A simulated lost reply after commit leaves the advanced
session receipt available; retrying the obsolete receipt fails. The caller must
read the current session state before deciding whether another action is needed.
No-op retries with that current receipt create no extra history.

Only immutable manifests persist in this increment. Selection, session revision
and undo/redo are **in memory**; reopening starts a new session with empty
history from an explicitly selected, freshly verified snapshot. The new tests
inject handled failures and reopen SQLite while retaining the session object.
They do not establish durable session recovery. The existing manifest
process-exit tests still cover snapshot persistence separately.

## Validation and next work

Seven new regressions cover session selection/edit/remove/undo/redo, no-ops and
branching, stale and cross-session receipts, revision overflow rollback,
invalid boundaries/dip conflicts, corrupt undo targets, same-revision project
changes, pre/post-commit faults, reopen lifecycle and bounded undo without
snapshot deletion. All 42 saved-adapter tests passed in the targeted run.
Evidence is beside the checkout under `audit-output/phase-j-session-*`.

The full local suite passed **391 native tests** (eight explicit acceptance modes
not selected), plus **332 frontend tests** across 65 files. TypeScript/Vite build
passed with the existing chunk-size warning. Clippy with warnings denied,
relevant Rust formatting and diff whitespace checks passed. No installed-app or
real-footage acceptance is claimed for this test-only session increment.

Next, define durable session checkpoints and explicit resume/recovery with
revision conflicts and stale-project handling. Then map selected mixed-layer
plans into the shared preview/seek clock. Production integration still requires
authorized root/source verification, versioned migration/import and old-client
rejection, reviewed compositor/audio policies, real-footage preview/export
parity and installed-Windows acceptance.

The original video-only draft/recovery format and XMP 0.56.76 handoff remain
preserved. No production activation, installer, installation, push, upload or
remote CI is included in this local update.

The subsequent 0.56.82 [checkpoint groundwork](PHASE_J_SESSION_CHECKPOINTS.md)
adds explicit persistence and resume/recovery of session selection/history.
Applying an edit still does not automatically checkpoint. The results above
remain this document's 0.56.81 in-memory session milestone.
