# XMP step 1 review — November 6, 2026

Scope: current local working tree in integration-source, including the new
XMP transactions, native command routing, saved-workspace authorization and
recovery UI. This is a review checkpoint, not an approval to enable replacement
or release. Production behavior has not been changed by this review. Added
three test-only characterization cases and this report; all work remains local.

## Findings

### P2 — A corrupt record blocks every otherwise healthy pending review

`src-tauri/src/xmp_transactions.rs:291–317`: list() returns one error for the
entire collection if any row fails its hash check. The UI then receives no
healthy rows. A two-record fixture with one damaged candidate confirms that
the second valid pending operation survives on disk but cannot be reviewed or
resolved through the screen. The bad record must remain preserved and clearly
flagged while independent healthy records remain usable. Do not silently drop
corrupt rows or rewrite their hashes to make verification pass.

### P2 — Cancellation depends on the original source still being available

`src-tauri/src/xmp_transactions.rs:415–422`: both keep-external and cancel first
acquire the full media/sidecar Guard. Moving or removing the photo makes that
open fail even though cancellation only archives the journal record and is
explicitly promised to leave all media/recovery files untouched. The operation
remains pending with no in-app way to dismiss it. A disposable-photo rename
reproduces the blocked cancellation while sidecar and candidate remain intact.

Separate journal-only cancellation from filesystem-changing actions. Preserve
workspace ownership checks and all snapshots. Keeping/applying a version can
retain stricter current-file validation; cancellation should not need folder
creation rights or a readable original. Removed roots, unavailable drives,
read-only folders and malformed sidecars need explicit behavior rather than
being permanent recovery dead ends.

### Recovery design gap — Published records are never reconciled or surfaced

`src-tauri/src/xmp_transactions.rs:292,568–573`: publication acknowledgement
sets state=published, and the sole review query permanently excludes that state.
If the destination later goes missing, the saved candidate remains in SQLite
but is not available through recovery controls. A fixture acknowledges a new
sidecar, removes its directory entry, and confirms that review remains empty
while the complete candidate still exists in the journal.

This is a missing-destination simulation, **not a reproduction of power loss**.
Intentional external deletion must not trigger automatic restoration. Provide
a read-only history/reconciliation path that distinguishes observed missing,
changed, inaccessible and unchanged destinations without guessing their cause.
The code currently flushes staged bytes before rename and commits SQLite after
publication; neither that ordering nor current tests prove cross-store
power-loss durability. Keep installed/power-loss acceptance open.

## Step 2 prerequisites

- Bind approval to operation ID, exact candidate hash and original media
  identity, registered-root identity and current destination identity/content.
  The current fingerprint covers only sidecar identity/content. Original media
  identity is not persisted in the operation record. It is sufficient for the
  current nondestructive decisions, but cannot be reused unchanged as an Apply
  authorization token when source media or proposals may have changed.
- Treat missing destinations as a separate create-new case. An absence snapshot
  is not a lock; a competing new sidecar must win without being overwritten.
- Do not release the existing-sidecar guard and then replace by pathname while
  claiming conflict prevention. Preserve/refuse if a verified publication
  primitive cannot maintain the invariant. Keep source originals read-only.
- Resolve the first two recovery findings before adding an Apply button; define
  history/reconciliation and durability limits as part of the transaction design.
- Test active writable handles, pathname replacement, staging swaps, source
  replacement, changed root identity, revoked registration and in-place reparse
  changes at the actual production publication boundary. A pre-existing static
  junction test is not proof against a live directory reparse mutation.
- Repeat fault/abrupt-process restart tests for each new durable state and keep
  ambiguous snapshots. Installed Windows and real-editor tests remain separate.

## Verification

Three review-only native cases reproduced the findings on disposable fixtures:

- audit_one_corrupt_record_hides_other_valid_pending_reviews
- audit_cancel_is_blocked_when_source_media_has_moved
- audit_missing_acknowledged_sidecar_is_not_exposed_to_recovery

The assertions deliberately characterize the current gaps. Passing them does
not mean the gaps are fixed; replace them with desired-behavior regression
assertions when implementing the fixes. Evidence is in
`../audit-output/xmp-step1-review-tests.log`.

The existing suite already covers conservative refusal of existing-sidecar
replacement, competing creation, stale sidecar decisions, journal/staging
corruption, saved-root removal, static NTFS junction refusal and synthetic
interruption/restart. No claim is made that those tests establish safe automatic
replacement or all in-place directory-reparse behavior.

## Handoff

Ready to start step 2's implementation design with the constraints above.
**Not ready to enable Apply or ship automatic replacement.** This review stops
before step 2. No commit, upload, remote CI, release or installed-app change was
performed. Existing uncommitted work and the earlier untracked review file were
preserved.

Final review checks: all 17 XMP transaction/review tests passed; Clippy with
warnings denied, formatting and diff whitespace checks passed. This includes
three successful reproductions of unresolved gaps, not three fixes. The full
application/frontend suites were not repeated because this step changes only
review documentation and test-only code.

## Step 2 follow-up

The three reproduced recovery findings have desired-behavior regression tests
and local fixes. Reviewed Apply and its remaining limits are documented in
[XMP_REVIEWED_APPLY.md](XMP_REVIEWED_APPLY.md). The review above records the state
before those fixes; it is retained as historical evidence rather than rewritten.
