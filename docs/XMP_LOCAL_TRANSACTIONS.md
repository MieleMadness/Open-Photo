# Local XMP transactions and review

Current local checkpoint: **0.56.76**. See [XMP closeout](XMP_CLOSEOUT.md) for the
final audit, test evidence, updated installer/source handoff and acceptance gates.
Local implementation is complete after its final checks; installed and release
acceptance remain separate. No original media is opened for writing.

## Current behavior

- New sidecars use no-overwrite publication. Foreign XMP is skipped; managed
  merges require fresh comparison and explicit Apply confirmation.
- Saved-workspace authorization, protected NTFS file/folder identities and full
  source-content checksums bind each operation. Missing legacy bindings are not
  silently backfilled. Complete intents are rechecked before filesystem mutation.
- FULL-sync SQLite records retain original/candidate versions and interruptions.
  A verified recovery original can be explicitly restored only to an empty path.
  Competing destinations are never overwritten; no winner is selected automatically.
- Writes and decisions run on background workers under the same save lock. The
  interface pages summaries, lazily loads documents and retains stored versions
  when the current source/folder is unavailable.
- Staging observations are advisory metadata only. Matching names are not proof
  of ownership or usable content and never authorize promotion or cleanup.
- Journal-only cancellation applies to ordinary pending proposals, including
  unavailable/removed roots in a still-saved workspace. Interrupted saves keep
  their stricter explicit recovery rules. Completed history remains inspectable.

The old sidecar moves to a protected recovery name before the new candidate is
published to a free destination name. This is a two-phase save with a possible
brief absent-name interval, not an atomic replacement or proof of power-loss
namespace durability. Supported writes remain limited to guarded local fixed NTFS.

## Historical implementation evidence

| Checkpoint | Detail |
| --- | --- |
| Step 1 | [Initial audit](XMP_STEP_1_REVIEW.md) |
| Step 2 | [Reviewed Apply and protected original](XMP_REVIEWED_APPLY.md) |
| Step 3 | [Real ExifTool/process-exit and explicit restore trials](XMP_STEP_3_VALIDATION.md) |
| 0.56.70 | [Earlier installer handoff](XMP_STEP_4_HANDOFF.md), preserved unchanged |
| 0.56.71 | [Full-source checksum binding and fixtures](XMP_SOURCE_CONTENT_VALIDATION.md) |
| 0.56.72 | [Background workers and serialized decisions](XMP_BACKGROUND_WORKERS.md) |
| 0.56.73 | [Paging and lazy documents](XMP_REVIEW_PAGING.md) |
| 0.56.74 | [Offline stored-document inspection](XMP_OFFLINE_REVIEW.md) |
| 0.56.75 | [Read-only staging observations](XMP_STAGING_OBSERVATIONS.md) |

Those documents retain their original test counts and limitations. The current
closeout supersedes stale descriptions; it does not retroactively establish
installed acceptance, remote CI or public release for any earlier candidate.
