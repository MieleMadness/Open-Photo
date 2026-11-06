# Read-only staging observations — local 0.56.75

An inspected XMP review now has a **Possible staging files** disclosure. Failed
saves intentionally preserve temporary stages, but their generated filenames
were previously absent from the review. The disclosure shows matching names,
entry types, observed regular-file sizes and inspection errors beside the saved
documents. It provides no cleanup, promotion or recovery action for those entries.

## What the observation establishes

After workspace/root authorization and protected parent acquisition, the current
review command considers filenames beside that media file. Only the strict
`.openphoto-xmp-{pid}-{operation-id}-{timestamp}.tmp` pattern with this operation's
exact ID is included. PID and timestamp fields must be numeric and in range.
Other operation IDs, malformed names and legacy recovery names are excluded.

This is **filename and metadata evidence only**. Contents and ownership are not
verified: another profile or program can create a matching name. Regular files
are marked contents-not-checked; folders are not searched, and links / Windows
reparse entries are not followed. The helper never reads staged contents or
retains data-file sharing locks that could interfere with publication. Hard-link
ownership is not determined by this metadata-only listing.

Observation happens only in the read-only current-review response, under the
existing directory guards. It is not performed by the Apply/restore authorization
path, does not participate in approval fingerprints and does not change the
protocol's treatment of retained stages. The independently verified recovery
original and its explicit restore action remain separate.

## Bounded and incomplete results

The scan examines at most 4,096 folder entries and retains at most 20 matches,
using a boundary lookahead to report additional entries/matches. Limits and
directory iteration errors are explicitly reported as incomplete; individual
metadata failures retain the filename with an unavailable status. The UI does
not call a partial or failed scan empty. A complete empty observation says only
that no matching names were seen in this check.

Names and sizes can change after observation. There is no guarantee that the
same entries still exist when the disclosure is opened. If the authorized current
folder cannot be acquired, no inventory is supplied and the interface makes no
claim that staging files were checked. Stored-document offline inspection from
[0.56.74](XMP_OFFLINE_REVIEW.md) remains available under its existing rules.

## Local validation

- **345 default native tests passed**, including seven new inventory/integration
  regressions. Eight explicit modes were ignored in that run; the real ExifTool
  13.61 matrix was separately selected and passed against 0.56.75. Seven other
  explicit acceptance modes remain unrun.
- **332 frontend tests passed** across 65 files, including 31 XMP interface tests
  and four new staging-disclosure cases.
- TypeScript/Vite build, Clippy with warnings denied, Rust formatting and diff
  whitespace checks passed. The existing frontend bundle-size warning remains.

Evidence is stored beside the checkout under `audit-output/xmp-staging-*`.
The new native tests cover strict filename scope, bounded scans, errors, directory
non-traversal and an actual Windows junction whose target remains unchanged.
Integration tests exercise a handled post-stage-write failure, preserved original
and retry, plus an actual child-process exit after staging flush followed by two
cold reopens and explicit original restoration. Stages remain intact, and the
advisory observation does not change approval fingerprints.

Frontend tests cover unverified entry labels, no file actions, partial/error/empty
states and absent observations. Headless Edge checks cover keyboard expansion,
the same states, original-restore confirmation, no automatic mutations and
light/dark 1220×780 plus dark 800×580 layouts. Screenshots were visually inspected
for long generated names, wrapping, normal text size and focus treatment without
horizontal overflow. This is browser-fixture evidence, not installed WebView2 QA.

## Remaining boundaries

This does not authenticate stages, restore from them, clean up old files, repair
the journal or prove power-loss durability. The no-overwrite publication and
protected-source rules remain unchanged. Installed real-media and lifecycle
acceptance, storage/power-loss trials and exact-source remote CI are still pending.
The 0.56.70 installer/snapshot is unchanged; no installation or upload is performed.
