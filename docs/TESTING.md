# Compatibility and regression policy

## Immich target

- Initial integration target: **Immich 3.3.5** (reported by initial tester).
- Open-Photo currently does not connect to Immich.
- Do not presume that XMP face names automatically map to existing Immich people. Test on an isolated instance using copied or synthetic images first.
- Record each API endpoint, minimum permission, example fixture and tested server version before enabling write or delete operations.

## Regression gates

Every pull request must pass:
1. Frontend typechecking and production bundle.
2. Rust formatting (`cargo fmt --check`), Clippy with warnings treated as errors.
3. Rust unit and regression tests.
4. Windows desktop installer packaging and artifact upload.

Do not merge a failing PR. A green build verifies the checks that ran; it does **not** replace manual testing of the installed application.

## Test priorities

- Scanner: missing and empty folders, mixed files, uppercase extensions, nested folders, symlink boundaries, unreadable directories, scan caps and file preservation.
- Metadata (later): XMP read/write round trips, preservation of unknown metadata, backup/recovery and crash interruption, malformed input.
- People (later): distinct identities with identical display names, no unreviewed merges, face region coordinate mapping.
- Immich (later): 3.3.5-specific tag, caption and face-region import fixtures; existing-person assignment using supported public API only.
- UI (later): accessibility, light/dark/system themes and folder selection smoke tests.

## Reproducibility improvements still required

Before release, commit `package-lock.json` and `Cargo.lock`, use `npm ci`, and document pinned toolchain/action versions. Avoid changing a workflow solely to suppress a failing test.

## Photo viewer MVP regression coverage

- Default is icon grid. Users can switch to list view.
- Sort modes: newest, oldest, filename A-Z, largest, smallest.
- Grid slider changes tile dimensions without modifying files.
- Selecting an image opens a larger on-demand preview.
- Backend refuses preview paths outside the canonical selected root.
- Files unsupported by the local decoder show a clear preview-unavailable state.
- Large file protection: files above 75 MB are not decoded in the current MVP.
- Image previews are generated locally; no image bytes are uploaded or written to the originals.
- Follow-up performance hardening: virtualized grid, thumbnail cache, decoded-pixel allocation limits, cancelable background decode.

## MVP gallery follow-up implementation

- Bounded, in-process cache keyed by canonical file path, size, modification time and requested resolution; no original file writes.
- Virtualized grid renders a window of visible rows rather than all files. This first implementation needs performance testing on very large libraries and with resized windows.
- Checkboxes support multi-selection across grid/list layouts; selection is reset when changing library folders.
- Explicitly confirmed batch operation creates **missing** `.jpg.xmp`-style sidecars with standard `dc:subject` keywords; existing sidecars are skipped.
- Regression coverage: repeat application does not overwrite the first sidecar and original image bytes are unchanged.
- Future work: metadata-aware merge for existing sidecars, tag removal, undo, filesystem event refresh, UI interaction tests, thumbnail disk-cache migration and EXIF display. Until metadata merge is tested against Immich 3.3.5, **do not** overwrite existing XMP.

## Incremental media-index milestone

- `scan_indexed` persists a per-root snapshot in the user's local Open-Photo SQLite data directory.
- Complete scans replace the previous index snapshot transactionally. Truncated scans are returned but not persisted as complete snapshots.
- `find_exact_duplicates` groups candidates by file size, then streams SHA-256. No delete or rename operations are exposed.
- The current duplicate UI compares files **within each folder root**; cross-root detection and persistent hash reuse remain planned.
- The index is currently storage groundwork, not an incremental directory watcher: sync still traverses roots. Implement change-only updates before claiming incremental rescanning.
- The initial exact-duplicate unit test verifies same-content/different-content fixtures. Expand coverage for media renames, root overlap, inaccessible files, and cross-root duplicates.
- Immich 3.3.5 connectivity and semantic AI search are still future work.
