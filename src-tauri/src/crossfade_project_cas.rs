//! Test-only proof of atomic draft CAS against the authoritative video sequence
//! revision. SQLite is in-memory; no production schema, command or file changes.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

fn database() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE video_sequences (
            id TEXT PRIMARY KEY, revision INTEGER NOT NULL, data TEXT NOT NULL
        );
        CREATE TABLE crossfade_drafts_test (
            project_id TEXT PRIMARY KEY, project_revision INTEGER NOT NULL,
            revision INTEGER NOT NULL, data TEXT NOT NULL
        );
        CREATE TABLE crossfade_draft_history_test (
            id INTEGER PRIMARY KEY, project_id TEXT NOT NULL,
            revision INTEGER NOT NULL, data TEXT NOT NULL
        );",
    )
    .unwrap();
    db
}

fn save(
    db: &mut Connection,
    project_id: &str,
    project_revision: i64,
    expected_draft_revision: Option<i64>,
    next: &str,
) -> Result<i64, String> {
    if project_id.is_empty()
        || project_id.len() > 120
        || !project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || project_revision < 1
        || next.is_empty()
        || next.len() > 131_072
    {
        return Err("Invalid crossfade draft update.".into());
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    // The authoritative sequence row, not the draft's own saved copy, owns
    // project freshness. This read and all writes share the same transaction.
    let current_project_revision: Option<i64> = tx
        .query_row(
            "SELECT revision FROM video_sequences WHERE id=?1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if current_project_revision != Some(project_revision) {
        return Err("Video project changed; reload crossfade draft.".into());
    }
    let existing: Option<(i64, i64, String)> = tx
        .query_row(
            "SELECT project_revision, revision, data
             FROM crossfade_drafts_test WHERE project_id=?1",
            params![project_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let next_revision = match (existing, expected_draft_revision) {
        (None, None) => {
            tx.execute(
                "INSERT INTO crossfade_drafts_test
                 (project_id, project_revision, revision, data)
                 VALUES (?1, ?2, 1, ?3)",
                params![project_id, project_revision, next],
            )
            .map_err(|error| error.to_string())?;
            1
        }
        (Some((bound_project, old_revision, old_data)), Some(expected))
            if bound_project == project_revision && old_revision == expected =>
        {
            if old_data == next {
                return Ok(old_revision);
            }
            let revision = old_revision
                .checked_add(1)
                .filter(|revision| *revision > 0)
                .ok_or("Crossfade draft revision overflow.")?;
            tx.execute(
                "INSERT INTO crossfade_draft_history_test
                 (project_id, revision, data) VALUES (?1, ?2, ?3)",
                params![project_id, old_revision, old_data],
            )
            .map_err(|error| error.to_string())?;
            tx.execute(
                "UPDATE crossfade_drafts_test SET revision=?1, data=?2
                 WHERE project_id=?3 AND project_revision=?4 AND revision=?5",
                params![revision, next, project_id, project_revision, expected],
            )
            .map_err(|error| error.to_string())?;
            tx.execute(
                "DELETE FROM crossfade_draft_history_test
                 WHERE project_id=?1 AND id NOT IN (
                   SELECT id FROM crossfade_draft_history_test
                   WHERE project_id=?1 ORDER BY id DESC LIMIT 20
                 )",
                params![project_id],
            )
            .map_err(|error| error.to_string())?;
            revision
        }
        _ => return Err("Crossfade draft changed; reload it.".into()),
    };
    tx.commit().map_err(|error| error.to_string())?;
    Ok(next_revision)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> Connection {
        let db = database();
        db.execute(
            "INSERT INTO video_sequences(id, revision, data) VALUES ('p', 5, '{}')",
            [],
        )
        .unwrap();
        db
    }

    fn draft(db: &Connection) -> Option<(i64, i64, String)> {
        db.query_row(
            "SELECT project_revision, revision, data
             FROM crossfade_drafts_test WHERE project_id='p'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .unwrap()
    }

    #[test]
    fn rejects_absent_and_stale_authoritative_project_revisions() {
        let mut db = seeded();
        assert!(save(&mut db, "missing", 5, None, "a").is_err());
        assert!(save(&mut db, "p", 4, None, "a").is_err());
        assert_eq!(save(&mut db, "p", 5, None, "a"), Ok(1));
        db.execute("UPDATE video_sequences SET revision=6 WHERE id='p'", [])
            .unwrap();
        assert!(save(&mut db, "p", 5, Some(1), "new").is_err());
        assert!(save(&mut db, "p", 6, Some(1), "new").is_err());
        assert_eq!(draft(&db), Some((5, 1, "a".into())));
    }

    #[test]
    fn accepts_new_draft_only_when_both_authoritative_revisions_match() {
        let mut db = seeded();
        assert_eq!(save(&mut db, "p", 5, None, "a"), Ok(1));
        assert!(save(&mut db, "p", 5, None, "b").is_err());
        assert!(save(&mut db, "p", 5, Some(0), "b").is_err());
        assert_eq!(save(&mut db, "p", 5, Some(1), "b"), Ok(2));
        assert_eq!(draft(&db), Some((5, 2, "b".into())));
    }

    #[test]
    fn failure_to_record_history_rolls_back_same_transaction() {
        let mut db = seeded();
        save(&mut db, "p", 5, None, "before").unwrap();
        db.execute_batch(
            "CREATE TRIGGER history_error BEFORE INSERT ON crossfade_draft_history_test
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
        )
        .unwrap();
        assert!(save(&mut db, "p", 5, Some(1), "after").is_err());
        assert_eq!(draft(&db), Some((5, 1, "before".into())));
    }

    #[test]
    fn no_op_preserves_current_revision_and_history() {
        let mut db = seeded();
        save(&mut db, "p", 5, None, "same").unwrap();
        assert_eq!(save(&mut db, "p", 5, Some(1), "same"), Ok(1));
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM crossfade_draft_history_test",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }
}
