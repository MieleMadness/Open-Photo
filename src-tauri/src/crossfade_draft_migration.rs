//! Test-only migration rehearsal against the real video_sequences table shape.
//! No production connection invokes this schema or changes existing projects.
use rusqlite::{params, Connection, OptionalExtension};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS crossfade_drafts_v1_test (
    project_id TEXT PRIMARY KEY,
    root TEXT NOT NULL,
    project_revision INTEGER NOT NULL CHECK(project_revision > 0),
    draft_revision INTEGER NOT NULL CHECK(draft_revision > 0),
    data TEXT NOT NULL CHECK(length(data) BETWEEN 1 AND 131072),
    updated INTEGER NOT NULL,
    FOREIGN KEY(project_id) REFERENCES video_sequences(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS crossfade_draft_history_v1_test (
    id INTEGER PRIMARY KEY,
    project_id TEXT NOT NULL,
    draft_revision INTEGER NOT NULL CHECK(draft_revision > 0),
    data TEXT NOT NULL,
    FOREIGN KEY(project_id) REFERENCES video_sequences(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS crossfade_draft_history_project_idx_test
    ON crossfade_draft_history_v1_test(project_id, id DESC);
";

fn initialize(db: &Connection) -> Result<(), String> {
    // Must only be called once video_sequences is available in the same DB.
    let exists: Option<i64> = db
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='video_sequences'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if exists.is_none() {
        return Err("Crossfade migration requires the existing video project table.".into());
    }
    db.execute_batch(SCHEMA).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn existing_database() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            r#"PRAGMA foreign_keys=ON;
             CREATE TABLE video_sequences (
                id TEXT PRIMARY KEY,
                root TEXT NOT NULL,
                title TEXT NOT NULL,
                revision INTEGER NOT NULL,
                data TEXT NOT NULL,
                updated INTEGER NOT NULL
             );
             CREATE TABLE video_sequence_history (
                id INTEGER PRIMARY KEY,
                project_id TEXT NOT NULL,
                revision INTEGER NOT NULL,
                data TEXT NOT NULL
             );
             INSERT INTO video_sequences
                (id, root, title, revision, data, updated)
                VALUES ('project-1', 'C:/library', 'Legacy project', 7, '{\"schema_version\":3}', 100);
             INSERT INTO video_sequence_history
                (project_id, revision, data)
                VALUES ('project-1', 6, 'old project');"#,
        )
        .unwrap();
        db
    }

    #[test]
    fn migration_is_idempotent_and_does_not_change_existing_project_bytes() {
        let db = existing_database();
        let before: (String, u64, String) = db
            .query_row(
                "SELECT data, revision, title FROM video_sequences WHERE id='project-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        initialize(&db).unwrap();
        initialize(&db).unwrap();
        let after: (String, u64, String) = db
            .query_row(
                "SELECT data, revision, title FROM video_sequences WHERE id='project-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(before, after);
        let old_history: String = db
            .query_row(
                "SELECT data FROM video_sequence_history WHERE project_id='project-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(old_history, "old project");
        let draft_count: u64 = db
            .query_row("SELECT COUNT(*) FROM crossfade_drafts_v1_test", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(draft_count, 0);
    }

    #[test]
    fn rejects_orphan_draft_and_invalid_revision_or_payload() {
        let db = existing_database();
        initialize(&db).unwrap();
        for (project_id, revision, draft_revision, payload) in [
            ("missing", 7, 1, "draft"),
            ("project-1", 0, 1, "draft"),
            ("project-1", 7, 0, "draft"),
            ("project-1", 7, 1, ""),
        ] {
            assert!(db
                .execute(
                    "INSERT INTO crossfade_drafts_v1_test
                     (project_id, root, project_revision, draft_revision, data, updated)
                     VALUES (?1, 'C:/library', ?2, ?3, ?4, 100)",
                    params![project_id, revision, draft_revision, payload],
                )
                .is_err());
        }
    }

    #[test]
    fn preserves_old_sequence_and_cascades_only_deleted_draft_data() {
        let db = existing_database();
        initialize(&db).unwrap();
        db.execute(
            "INSERT INTO crossfade_drafts_v1_test
             (project_id, root, project_revision, draft_revision, data, updated)
             VALUES ('project-1', 'C:/library', 7, 1, 'draft', 100)",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO crossfade_draft_history_v1_test
             (project_id, draft_revision, data) VALUES ('project-1', 1, 'recovery')",
            [],
        )
        .unwrap();
        db.execute("DELETE FROM video_sequences WHERE id='project-1'", [])
            .unwrap();
        let count: u64 = db
            .query_row("SELECT COUNT(*) FROM crossfade_drafts_v1_test", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
        let history_count: u64 = db
            .query_row(
                "SELECT COUNT(*) FROM crossfade_draft_history_v1_test",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(history_count, 0);
        // Production sequence_history has no new foreign key or change.
        let previous_history: String = db
            .query_row("SELECT data FROM video_sequence_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(previous_history, "old project");
    }

    #[test]
    fn migration_fails_closed_without_authoritative_project_table() {
        let db = Connection::open_in_memory().unwrap();
        assert!(initialize(&db).is_err());
        let created: Option<String> = db
            .query_row(
                "SELECT name FROM sqlite_master WHERE name='crossfade_drafts_v1_test'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap();
        assert_eq!(created, None);
    }
}
