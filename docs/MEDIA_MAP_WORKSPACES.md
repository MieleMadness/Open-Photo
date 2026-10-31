# Open-Photo media, map, and workspace MVP scope

## Implemented in development branch (subject to CI and real-user testing)

- Named local workspaces saved in application settings. Each workspace points to its own filesystem root.
- Folder browsing based on discovered file paths, with navigation back into All Photos.
- Full-height sidebar and removed privacy callout.
- Discovery of MOV, MP4, M4V, AVI, MKV and WebM alongside image formats.
- Map view for EXIF GPS coordinates in JPEG/TIFF, OpenStreetMap tiles and clickable photo markers.

## Known limitations and next steps

1. **HEIC/MOV metadata:** parse ISO BMFF/QuickTime GPS fields; test with iPhone originals including Live Photo companions and timezone information.
2. **Video playback:** add a safe local media strategy; account for HEVC/H.265 codecs, iPhone MOV rotation, 4K playback, thumbnails and hardware acceleration. Merely recognizing an extension does not mean the codec is supported.
3. **Map clustering:** cluster dense locations, selection by viewport, filters by media/date/workspace, privacy controls for sensitive coordinates. Map tiles require network connectivity, but location extraction remains local.
4. **Workspaces:** migrate workspace selection from browser localStorage to durable app-managed storage, guard concurrent background scans and supply rename/delete/export workspace controls.
5. **Folders:** distinguish physical folders from virtual collections/albums. Use non-destructive virtual collections as the default grouping mechanism.
6. **MVP reliability:** persist thumbnail cache with eviction, introduce cancellable tasks, test a 10k+ item workspace; store stable media IDs to survive renames and moves.
7. **Immich 3.3.5:** validate XMP keywords/face regions on a separate test server; never connect directly to Immich PostgreSQL or modify managed files.
8. **Regression coverage:** GPS edge cases (S/W hemispheres, missing refs, invalid coordinates), read-only boundaries, workspace isolation, video corruption and file permissions, map marker-to-photo association.

Release policy: do not merge a red Windows workflow or claim playback works until its native codec path is implemented and verified.
