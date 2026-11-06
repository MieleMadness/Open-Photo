# Local XMP closeout — 0.56.76

Local XMP implementation and audit are **closed out**. Source validation, the
optimized release build and fresh 0.56.76 installer packaging passed. The delivery
is prepared for later acceptance; resource-input and delivery hash results are
recorded in its manifests. This is not an installed-app acceptance or release
declaration. No further XMP feature additions
are planned before acceptance feedback; the next coding track is Phase J video
transition groundwork, with its existing production-enablement gates retained.

## Implemented local behavior

- No-overwrite new-sidecar creation and explicit reviewed Apply for managed XMP;
  foreign documents are skipped and unrelated metadata is preserved.
- Full-source SHA-256, Windows source/root/sidecar identity and saved-workspace
  authorization; old proposals without required bindings must be recreated.
- Persisted intents, retained original/candidate snapshots, explicit original
  restoration into an empty destination and deliberate Keep/cancel decisions.
- Background workers with serialized writes/decisions, paged summaries and lazy
  documents, offline stored-document inspection and read-only staging observations.
- No original-media writes, forced overwrite, automatic winner selection, staging
  promotion or automatic cleanup of ambiguous recovery artifacts.

## Final audit finding and fix

A new deterministic regression reproduced a journal-consistency gap: a second
SQLite connection changed a saved source checksum after Apply committed intent,
but before its second transaction. The partial intent check accepted that change.
This was a disposable journal-mutation trial, not an external photo-editor trial
or evidence that original media was overwritten.

New-sidecar creation, Apply and Restore now compare the complete expected record
inside the transaction that performs publication/restoration. Bindings include
root/path, source identity/checksum, original/candidate contents and hashes,
sidecar/root identities, state, backup name and approval token. New-sidecar
creation also rechecks the registered-folder identity after its preparation
commit. Internal fields do not expand the frontend record contract; no schema
migration or source-media rewriting is introduced.

Four regressions cover 13 independent record changes on each of the three write
paths plus a changed folder binding (40 fault variations). They require refusal
before the original moves, a new sidecar appears or a restoration consumes its
backup. The failing pre-fix reproduction is retained separately from final results.
These checks do not authenticate a database against arbitrary privileged changes
outside the protected transaction or make the journal tamper-proof.

## Final evidence and handoff

Final formatted-source results: **349 default native tests**, **332 frontend
tests across 65 files**, Clippy with warnings denied, Rust formatting and diff
whitespace checks passed. The real ExifTool 13.61 matrix was separately selected
and passed. Eight modes were ignored in the default native run; seven other
explicit modes remain unrun. Four browser suites passed paging, offline review,
staging observations and confirmed restoration/history using synthetic data;
light/dark/narrow screenshots were visually inspected. These are not installed
WebView2 results. The existing frontend bundle-size warning remains.
Fresh NSIS packaging completed successfully (about 648 MiB). The full formatted
source was rebuilt after a module-order correction; the interrupted precursor
log is retained and clearly superseded by the successful final installer log.

Evidence is recorded in `audit-output/xmp-closeout-*` beside the checkout. The
final handoff directory is `audit-output/xmp-closeout-0.56.76`: updated installer,
source snapshot including uncommitted work, source/build-input hashes, resource
manifests, logs, fixture package, verification script and acceptance checklist.
The older 0.56.70 handoff remains unchanged and is superseded for new testing.
Git HEAD alone does not identify this candidate; use the source manifest and
installer SHA-256. Documentation can be finalized after compilation; non-document
source/build inputs must match their pre-build hashes.

Run `verify-handoff.ps1` before using the delivery. It checks local hashes without
launching or installing anything. Hash agreement identifies the supplied bytes;
it is not a publisher signature. The candidate is unpublished and no updater
artifacts or remote workflow dispatch are part of this closeout.

## Acceptance still open

| Gate | Status / action |
| --- | --- |
| Installed WebView2 XMP flow | Pending on disposable Windows with copied real JPEG/TIFF/HEIC and video fixtures |
| External-editor / root / permissions | Native synthetic/ExifTool evidence is separate from installed real-media and OS configuration trials |
| Installed AI/runtime and install/update/uninstall | Pending; never run destructive lifecycle checks on a personal profile |
| Release video export and remaining explicit modes | Record separately; ignored tests are not acceptance passes |
| Storage failures and power loss | Durability not established; sidecar destination can briefly be absent during the two-phase save |
| Remote CI / merge / publication | Not performed; exact-source CI and full Windows acceptance remain required before release |

The seven other explicit native modes are two AI/model acceptance modes, short
release-video export, supplied real-video acceptance, and three long/full/hour-scale
video modes. The XMP editor mode is selected separately rather than left unrun.
Owner-deferred long-form modes are not newly required to finish this narrow local
XMP phase; do not mistake their default skips for completed acceptance either.

`scripts/test-installed-app.ps1` remains restricted to disposable GitHub-hosted
Windows runners. Never spoof its environment guards. No installation is performed
on this computer. The acceptance/durability requirements tracked under Issue #90
remain unverified here; this local closeout does not close or resolve that issue.

## References

[Current behavior index](XMP_LOCAL_TRANSACTIONS.md),
[step 3 editor/recovery evidence](XMP_STEP_3_VALIDATION.md),
[source-content binding](XMP_SOURCE_CONTENT_VALIDATION.md),
[background workers](XMP_BACKGROUND_WORKERS.md),
[review paging](XMP_REVIEW_PAGING.md),
[offline review](XMP_OFFLINE_REVIEW.md),
[staging observations](XMP_STAGING_OBSERVATIONS.md).

Earlier documents and dated roadmap entries describe historical checkpoints.
Their old limits, test counts, PR status and intentionally ignored race gates
must not override this local checkpoint or be read as current remote CI status.
