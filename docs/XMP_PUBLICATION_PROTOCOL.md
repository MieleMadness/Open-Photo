# XMP publication protocol: engineering gates

Historical engineering proposal against the older writer. The current local
implementation and verified boundary are recorded in [XMP closeout](XMP_CLOSEOUT.md).
The previously ignored successful-publication race gate now exercises the
production no-overwrite protocol; these earlier design notes are not its status.
The installed/durability requirements from Issue #90 remain unverified locally.

## Evidence available

- The held read guard excludes FILE_SHARE_WRITE but includes FILE_SHARE_DELETE.
  New writable opens are refused while it is held.
- A pre-existing writable handle prevents acquiring that guard (PR #116).
- On the local Windows test machine, ReplaceFileW succeeded with the read guard
  held. PR #117 now requires complete staged destination bytes and consumption
  of the staged file on success. It retains successful-test diagnostics in CI.
- Consequently, excluding writable opens is insufficient evidence that the
  pathname cannot be replaced. A retained handle and the current pathname may
  refer to different files after a rename. Do not promote these experiments to
  production locking without the gates below.

## Synthetic verification sequence

Local synthetic coverage now exists for steps 1, 2 and 4, and for the hardlink
and parent-directory rename portions of step 3. Junction/reparse changes,
registered-root authorization and the production boundary remain pending.

1. Hold the guard, then have an independent process replace the destination
   using a separate staged file. Record the operation result, bytes read from
   the retained handle, and bytes read through the pathname. Use barriers for
   deterministic ordering and verify child-process exit status.
2. Repeat with delete sharing denied. Characterize both external rename and
   Open-Photo publication; do not introduce an unlock-before-replace window.
3. Exercise hardlink aliases and parent-directory rename/reparse changes.
   Compare handle-bound volume/file identities with the authorized workspace
   and current destination. Matching content alone is not identity proof.
4. Preserve the writer-first test, including acquisition refusal and successful
   retry after the writer closes. Include a separate process with an already
   open handle, rather than relying exclusively on same-process handles.

Every experiment uses disposable media and checks original-source hashes.
Unsupported filesystem behavior must cause refusal, not a weaker fallback.

## Required production boundary

The eventual implementation must bind authorization, destination identity and
the final expected-content comparison to the protected publication operation.
If Windows APIs and the chosen sharing policy cannot maintain that invariant,
refuse the update and preserve a reviewable candidate rather than releasing
the guard, checking again and claiming atomic conflict prevention.

The existing ignored regression
`external_edit_between_compare_and_successful_publish_must_not_be_lost` is a
release gate. Adapting its injection seam to a real locking implementation may
be necessary, but the competing editor's bytes must still survive or the edit
must be refused before it occurs. Do not simply remove its interleaving.

## Recovery and durability

Current stage and backup files are flushed with sync_all, but that alone does
not establish crash-durable publication or a restart transaction. Before
automatic recovery, define a persisted operation identity, expected original
identity/hash, candidate hash, and explicit stage/backup/publish/acknowledgement
states. Inject failure and restart at every boundary. Preserve ambiguous
artifacts and later external edits; require explicit user review to choose a
version. Never infer recoverability solely from file presence or timestamps.

## Acceptance order

Synthetic Windows process/identity tests, complete native/frontend checks,
combined-commit CI, then full Windows installer and installed NTFS trials.
Include network/removable filesystem refusal behavior. Passing fast CI is not
installed acceptance or power-loss evidence. Keep Phase J production crossfade
acceptance and the deferred pre-1.0 media stress test as separate gates.

## November 5 continuation

Added executable independent-process characterization for synthetic experiment
steps 1 and 2 in `xmp_windows_sharing_tests`: retained-handle versus pathname
bytes after external replacement, and external/own publication refusal with
delete sharing denied. See `XMP_WINDOWS_SHARING_EXPERIMENT.md` for exact scope.
These tests do not implement the production publication boundary. Additional
identity and writer-first coverage is recorded in the continuation below;
crash and installed gates remain open. Phase J acceptance remains separate.

## Local continuation: additional process and identity gates

The synthetic suite now covers step 4's writer-first independent process, plus
step 3's hardlink identity and parent-directory rename/rebinding cases. Identity
comparisons use GetFileInformationByHandle volume serial and file index through
live handles; identical content does not substitute for identity. The local
parent rename is refused while its child-file guard remains open; rebinding is
exercised only after releasing it. See the experiment document for boundaries.
Junction/reparse/root-authorization and production publication remain open.

## Local production routing checkpoint

See [XMP_LOCAL_TRANSACTIONS.md](XMP_LOCAL_TRANSACTIONS.md). The runtime command now
journals existing-sidecar proposals and refuses automatic replacement; the old
replace helper exists only in historical tests. New sidecars use verified owned
staging and native handle-relative no-overwrite publication. Recovery review,
registered-folder identity checks and fault/abrupt-process restart tests are
implemented locally. The conservative refusal path is not a usable automatic
replacement protocol; Issue #90 and installed/power-loss gates remain open.

## November 6 — reviewed Apply implementation

The local step 2 protocol is implemented in xmp_transactions: retain the old
sidecar under a protected-handle recovery rename, then publish the reviewed
candidate without overwriting any new destination. This is a two-phase namespace
change with a possible absence interval, not a single atomic replacement.
See [XMP_REVIEWED_APPLY.md](XMP_REVIEWED_APPLY.md) for exact semantics, review
bindings, recovered findings and local production-boundary evidence. Installed
and power-loss acceptance remain open; this is not a release or Issue #90 closure.

## Step 3 checkpoint

The race gate now executes the production interleaving without being ignored.
Explicit guarded original restoration, abrupt-process/repeated recovery checks
and a real ExifTool matrix passed locally. See
[XMP_STEP_3_VALIDATION.md](XMP_STEP_3_VALIDATION.md) for exact scope, the test-only
cleanup fix and remaining step 4 / durability gates. No release is authorized.
