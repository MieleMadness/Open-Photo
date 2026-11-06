# XMP publication protocol: engineering gates

Status: proposed verification sequence, not an implemented protocol. Issue #90
remains open. The current writer can overwrite an external edit between its
last content comparison and successful publication.

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

## Next synthetic experiments

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
