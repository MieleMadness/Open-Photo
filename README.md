# Open-Photo

**An offline-first photo organizer for Windows, built for portable metadata and optional Immich compatibility.**

> Early development: this repository is a project foundation, not a functional photo manager yet.

## Project goals

- Browse and organize photos on local drives without a cloud account.
- Preserve originals: scanning must be read-only, and metadata writes must require an explicit preview and confirmation.
- Prefer portable XMP sidecar metadata for keywords, hierarchical tags, captions, ratings, and compatible face regions.
- Detect faces locally, allow people to be named and reviewed, and export standardized region metadata where supported.
- Offer optional Immich synchronization in a later milestone. Importing face names does **not** guarantee that they match an existing Immich person, so this requires version-specific testing.
- Respect privacy: never send photos, face data, thumbnails, or metadata to remote services without a deliberate opt-in.

## Planned stack

- Tauri 2 desktop shell with React + TypeScript frontend
- Rust for filesystem access and background indexing
- SQLite for rebuildable indexes and task history
- ExifTool for metadata reading/writing after evaluating deployment and licensing
- Local face detection/recognition as an opt-in later phase

## Development roadmap

| Milestone | Scope |
| --- | --- |
| 0.1.0 | Bootstrap desktop application, automated Windows build and demo UI |
| 0.2.0 | Local folder selection, safe scanning, photo grid, EXIF inspection |
| 0.3.0 | Portable XMP tags and descriptions, write previews, atomic file updates |
| 0.4.0 | Local face detection and people review, XMP region export |
| 0.5.0 | Immich compatibility test suite and optional connector |

## Security and file safety

1. The application must never directly edit an Immich database or its managed storage.
2. No silent deletion or file mutation. Deletion must be off by default and use a deliberate confirmation flow.
3. Sidecar changes need a recoverable backup or atomic write with verification.
4. Scan untrusted files defensively; prevent path traversal and symbolic-link escapes.
5. Exclude API keys, private photos, databases, embeddings, and thumbnail caches from version control.
6. Test against synthetic or licensed images. Do not commit real family photos.

See [architecture](docs/ARCHITECTURE.md), [first milestone](docs/MILESTONES.md), and [contributing](CONTRIBUTING.md).

## License

The project is currently **private and not yet licensed for public reuse**. A public open-source license will be chosen before any public release. Keeping the repository private does not prevent development.
