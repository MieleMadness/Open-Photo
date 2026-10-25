# Milestones and acceptance criteria

## 0.1.0 — Desktop foundation

- [ ] Scaffold Tauri 2 + React + TypeScript and commit dependency lockfiles.
- [ ] Build a basic Library / People / Tags / Settings navigation UI with demo data clearly labeled.
- [ ] Configure Windows CI to build and test pull requests.
- [ ] Verify no file modifications are possible from demo mode.
- [ ] Document Windows developer setup and packaging.

## 0.2.0 — Local browsing

- [ ] Select local scan folders with OS picker.
- [ ] Read-only recursive scan with cancellable progress.
- [ ] Display thumbnails, searchable grid, and metadata details.
- [ ] Handle broken files, symlinks, long paths, and empty folders safely.

## 0.3.0 — Portable XMP

- [ ] Write tags and captions to XMP sidecars after showing a diff preview.
- [ ] Preserve unrelated metadata fields.
- [ ] Atomic writes with backup/recovery; undo where possible.
- [ ] Verify export/import on a copied ten-photo sample set.

## 0.4.0 — People

- [ ] Opt-in local face processing.
- [ ] Manual correction and safe person merge.
- [ ] Portable XMP face regions and naming.
- [ ] Never auto-merge people merely because display names match.

## 0.5.0 — Immich bridge

- [ ] Detect version and supported API capabilities.
- [ ] Connect via scoped API key held in secure OS storage.
- [ ] Validate tags, descriptions and face regions against test Immich.
- [ ] Preview person mappings and require manual confirmation for ambiguous matches.
- [ ] Never touch Immich storage directories or PostgreSQL directly.
