# Open-Photo architecture (initial design)

## Offline-first components

1. **Desktop UI:** Tauri 2 + React/TypeScript, with explicit controls for import, preview, and write actions.
2. **Local index:** SQLite stores discovered file paths, hashes, thumbnails, activity history, and application-specific identity mappings. The index is rebuildable and is not the sole source of user metadata.
3. **Metadata adapter:** Read EXIF/IPTC/XMP and write portable companion XMP sidecars. Protect existing fields by merging rather than replacing unknown metadata.
4. **People catalog:** Stable local UUID for each person, user-reviewed naming and face associations. Export MWG XMP regions where compatible. A local UUID does not automatically correspond to an Immich person ID.
5. **Optional Immich bridge:** Read-only discovery first. Version-gated synchronization will require explicit per-operation authorization.

## Data boundaries

- **Original media:** remain untouched unless the user expressly opts into a future embedded-metadata workflow.
- **XMP sidecar:** portable record for keywords, hierarchical tags, captions, ratings, and supported face regions.
- **SQLite:** thumbnails and cache, application state, manual people mappings, jobs, and history. Backup options are needed because not all app-specific state fits standardized XMP.
- **Immich:** independent application and database; never modify its database or managed file directories directly.

## Filesystem requirements

- Support paths with Unicode and long Windows filenames.
- Detect moved files, duplicate filenames, readonly folders, and missing sidecars.
- Resolve sidecar naming conventions and test against Immich before promising round-trip portability.
- Write via temp file + flush + atomic rename where supported; create recoverable backups and ensure interrupted writes do not corrupt existing sidecars.
- Avoid following links outside selected scan roots without approval.

## Privacy

The application must work without accounts or remote traffic. Opt-in face processing must happen locally. Any later API integration stores secrets in Windows Credential Manager or OS-backed secure storage, not in logs or SQLite plaintext.

## Immich interoperability validation

Automated fixtures and manual verification must test: plain and hierarchical tags; captions; dates; rating; face-region coordinates and names; matching existing people; and album differences. Feature support can vary by Immich version, and mismatches must be reported rather than silently merged.
