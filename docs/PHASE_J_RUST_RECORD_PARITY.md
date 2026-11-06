# Phase J — Test-only native draft-record parity

A Rust validator for the experimental TypeScript `crossfadeDraftRecord` v1 shape is compiled only under `#[cfg(test)]`. It checks field sets, exact project and ordered clip/frame identity, 30/60 fps, 200-clip and two-hour bounds, 64 KiB input, safe integer frames, 0.08–1.5 second transitions, adjacency, duplicate edges and the two-frame overlap reserve.

Rust regression fixtures mirror TypeScript acceptance/rejection examples. **This does not create an active native persistence API, export command, database table or project schema migration.** Invalid existing draft content must never be silently erased.

Before production integration, native and frontend validation should share interoperable recorded fixture bytes (including rational source mark identity, revision bound snapshots and future-version rejection), secure authenticated source verification, atomic revision-checked persistence/recovery and installed Windows real-media acceptance. The current hard-cut/dip editor remains the supported production path.
