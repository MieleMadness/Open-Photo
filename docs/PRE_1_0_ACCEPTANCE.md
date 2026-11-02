# Open-Photo 1.0 release acceptance gates

These must be completed before calling Open-Photo version 1.0 production-ready. A green GitHub CI run is necessary but not a substitute for installed Windows and real-media testing.

## Deferred Phase G test — explicitly required before 1.0

- [ ] Test an installed Windows build with **10,000 or more real photos and videos** (including Live Photos, HEIC and multiple formats where available).
- [ ] Measure startup, folder Sync, search/filter/sort and grid/list scrolling responsiveness; record hardware specifications and observed times.
- [ ] Test repeated rapid scrolling, switching views, and changing thumbnail sizes; watch memory use and ensure cached thumbnails remain bounded.
- [ ] Verify multi-select, open/close details, source relinking, and video previews while the library is under load.
- [ ] Confirm no changes to original media or foreign sidecars and no unexpected crashes or freezes.
- [ ] Document and fix any reproducible blockers before approving 1.0.

## Additional release gates (not yet signed off)

- [ ] Windows installed-build regression checks, including video editing, export and recovery from failed operations.
- [ ] Validate signed Preview installation and upgrade between two real versions.
- [ ] Phase H map testing on real JPEG/TIFF/HEIC/HEIF and MOV/MP4 GPS media (including Live Photo pairs), N/S/E/W and antimeridian/polar positions, malformed metadata, zoom clustering, filters, offline tiles/privacy and installed-Windows performance. Phase H feature code is complete; this acceptance is not waived.
- [ ] Review and complete remaining roadmap milestones or explicitly document nonblocking exclusions from 1.0 scope.

Owner decision: Phase G's 10,000-item real-library acceptance is deferred while Phase H development proceeds, **not waived**.
