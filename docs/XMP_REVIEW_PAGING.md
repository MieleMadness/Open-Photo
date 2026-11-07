# Paged XMP reviews — local 0.56.73

The Tags review surface now loads **up to 50 summaries per page**, with **Older
changes** and **Newer changes** controls. It no longer stops at the latest 1,000
operations or sends both stored XML documents for every listed row. Opening one
review loads its stored record and obtains the current source/sidecar snapshot.
The stored record is integrity-checked, and its candidate hash must still match
both the summary and the fresh snapshot before confirmation is available.

## Navigation and concurrency

Pages use descending operation IDs and an exclusive `before` cursor. New records
do not shift older-page boundaries or cause offset-based duplicates. Refresh
starts at the newest page. Each request observes current journal state: this is
not a frozen snapshot, and another decision can remove a record from the pending
filter while browsing. Empty older pages retain a route back and Refresh.

Only the current page and the cursors needed to navigate back are retained in
the interface. Page changes clear the selected review and its consent, then move
keyboard focus to the review heading when loading finishes. Workspace changes,
history toggles, explicit Refresh and successful decisions reset paging. Late
page/detail/snapshot results from a previous request are ignored. A page error
leaves Newer changes and Refresh available; it does not hide recovery actions
behind a permanent loading state or imply that no records exist.

## Native contract

- `xmp_review_page(workspace, history?, before?)` returns `items` (only ID, path,
  state and candidate hash) and `next_before`. It fetches one extra metadata row
  to detect another page. Workspace membership is required; invalid nonpositive
  cursors are refused. Both pending interruptions and completed history can be
  paged without a 1,000-record ceiling.
- `xmp_review_record(workspace, id)` returns one full stored, integrity-checked
  operation. It requires the saved workspace, not an online root/source. A
  different workspace cannot retrieve that operation. The independent current
  snapshot retains the existing filesystem authorization/protection checks.
- Additive indexes support workspace/ID ordering for all and pending records.
  No journal rows, source files, sidecars or recovery artifacts are deleted.
- The older `xmp_reviews` command remains available with its original 1,000-row
  result for compatibility. The current Tags interface uses the paged commands.

Summary rows intentionally do not promise that their documents have been
verified: corruption is reported when the selected record is read. A malformed
stored document does not force every healthy document to load before the page
can appear. This does not repair a damaged SQLite database.

## Verified local results

- **336 native tests passed**, including five new paging regressions. A synthetic
  1,205-operation journal was traversed to the oldest record, with workspace
  isolation, no duplicates and pages no larger than 50.
- Native checks covered interruption/history filters, fresh inserts, invalid
  cursors, offline record inspection, integrity checks, and summary queries that
  do not deserialize large or invalid XML fields.
- **321 frontend tests passed** across 65 files, including 20 XMP interface tests.
  Six new tests cover lazy loading, paging/consent reset, refresh/history/workspace
  reset, late pages, page failures and mismatched record details.
- TypeScript/Vite build, Clippy with warnings denied, formatting and diff
  whitespace checks passed. The existing frontend bundle-size warning remains.
- A headless Edge fixture passed keyboard paging/focus, details, approval reset,
  history and page-error recovery. Light/dark 1220×780 and dark 800×580 captures
  showed no horizontal overflow. Enabled paging-button text contrast was above
  15:1, and controls retained the shared 42px height. Visual inspection also
  checked long filenames, wrapped actions, theme focus and disabled controls.

Evidence is in `audit-output/xmp-paging-*` beside the checkout, including native,
frontend, lint, build and browser logs and three screenshots. Eight explicit
native acceptance modes were not run in this update. The ExifTool matrix last
passed separately for the 0.56.71 core; this update changes browsing, not the
publication protocol.

## Remaining boundaries

These are local source and browser-fixture results, not installed WebView2 or
large real-library acceptance. The 1,205-row journal fixture is not the deferred
10,000-media library stress test. Full-source verification still reads every
byte when obtaining a current snapshot or applying/restoring. No automatic
recovery or competing-file overwrite is introduced. The 0.56.70 installer and
its source snapshot remain unchanged. Nothing was installed, uploaded or
published; installed/lifecycle/power-loss acceptance and exact-source remote CI
remain pending.
