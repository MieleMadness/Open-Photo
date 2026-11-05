# Phase J — Additive crossfade storage migration rehearsal

This regression module rehearses a proposed separate crossfade-draft schema against a SQLite database with the **same existing video_sequences and video_sequence_history column shapes as production**. It checks that the new tables initialize twice without changing existing project JSON, revisions, titles or saved history. New draft rows are bound to the authoritative project ID; invalid revisions, orphan rows and empty records fail. Deleting a project cascades to only the new draft tables in this fixture.

**Not activated:** The proposed tables end in `_test` and exist only in a Rust `#[cfg(test)]` module. The production application's database initializer is not altered, there are no Tauri commands, and no schema v3 data is rewritten.

A production migration requires choosing supported old-client behavior and future recovery, ensuring every authoritative project mutation invalidates stale drafts, safe root/project ownership, actual source-fingerprint checks, bounded crash-safe history, user messaging, and installed Windows acceptance. `FOREIGN KEY` enforcement must be enabled for each database connection or equivalent explicit deletion cleanup used. A real migration also needs SQLite durability/backup policy and upgrade/downgrade tests.
