# Offline XMP document inspection — local 0.56.74

Stored XMP documents are now loaded independently of live media verification.
Opening a review first reads its saved operation, validates its ID, path and
candidate hash against the selected summary, then requests a current source /
sidecar check. The stored original and proposal can be read while that check
is running and remain visible if it fails. A failed document read does not start
a live media check or leave an earlier document displayed as the new record.

## Recovery and decisions

An unavailable current file is explicitly marked **could not be verified**. It
is not treated as an empty sidecar path. Apply, restoration and Keep current
actions remain unavailable without the corresponding fresh snapshot. The
recorded recovery-original filename can still be displayed for an interrupted
save; that name is not proof that its file exists or is safe to restore.

**Read current version again** reloads both the stored operation and current
check. A stored-document failure instead offers **Read saved versions again**.
Retries clear consent, and a successful check still requires fresh confirmation
before Apply/restoration. Version or record-identity mismatches block the review
and require Refresh. A late stored record from an old workspace/request is
discarded before it can start a current-file check.

Journal-only cancellation remains available for an ordinary pending proposal
when its source/folder cannot be read or the root was removed from a still-saved
workspace. It never modifies source media or sidecars. Interrupted saves do not
gain a Cancel action; they retain their existing explicit recovery rules. Deleting
the whole workspace denies record access and decisions rather than granting
another workspace ownership of its journal.

## Verified local results

- **338 native tests passed**, including two new in-memory regressions for root
  removal, retained original/proposal documents, journal-only cancellation and
  denial after workspace deletion. Clippy with warnings denied passed.
- **328 frontend tests passed** across 65 files, including 27 XMP interface tests.
  Seven new cases cover documents during slow verification, offline inspection /
  retry, saved-record failures, old-workspace replies, interrupted backup references
  and mismatched record IDs/paths. Existing cancellation assertions now check that
  both stored documents remain readable when the current check fails.
- TypeScript/Vite build, Rust formatting and diff whitespace checks passed.
  The existing frontend bundle-size warning remains.
- A headless Edge fixture passed pending/interrupted offline inspection, slow
  checks, error handling, fresh-consent retry and the absence of automatic
  mutations. Light/dark 1220×780 and dark 800×580 captures were visually inspected
  with long filenames, expanded original documents, wrapping controls and no
  horizontal overflow. The unavailable-file notice uses normal body text size.

Evidence is in `audit-output/xmp-offline-*` beside the checkout. Eight explicit
native acceptance modes were not run in this update. The ExifTool matrix last
passed separately against the 0.56.71 core; this update changes the interface
and tests, not the source-checksum or sidecar-publication protocol.

## Remaining boundaries

This does not grant filesystem access to removed roots, repair corrupt documents,
rebind old source checksums, or automatically restore a missing file. Current
verification still requires the existing protected reads and can take time on
large files. Browser fixtures do not establish installed WebView2 acceptance.
Installed real-media/lifecycle tests, power-loss durability and exact-source
remote CI remain pending. The preserved 0.56.70 installer/snapshot is unchanged;
nothing was installed, uploaded or published.
