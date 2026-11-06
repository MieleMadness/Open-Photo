//! Phase J test-only SQLite compare-and-swap draft persistence prototype.
//!
//! All tests use Connection::open_in_memory(). No production schema, DB
//! migration, saved project, original media or Tauri command is modified.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

const MAX_DRAFT_BYTES: usize = 131_072;

fn initialize(db: &Connection) -> Result<(), String> {
    db.execute_batch(
        "CREATE TABLE crossfade_drafts_test (
            project_id TEXT PRIMARY KEY,
            project_revision INTEGER NOT NULL,
            draft_revision INTEGER NOT NULL,
            data TEXT NOT NULL
        );
        CREATE TABLE crossfade_draft_history_test (
            id INTEGER PRIMARY KEY,
            project_id TEXT NOT NULL,
            draft_revision INTEGER NOT NULL,
            data TEXT NOT NULL
        );",
    )
    .map_err(|error| error.to_string())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Stored {
    project_revision: u64,
    draft_revision: u64,
    data: String,
}

fn read(db: &Connection, project_id: &str) -> Result<Option<Stored>, String> {
    db.query_row(
        "SELECT project_revision, draft_revision, data
         FROM crossfade_drafts_test WHERE project_id=?1",
        params![project_id],
        |row| {
            Ok(Stored {
                project_revision: row.get(0)?,
                draft_revision: row.get(1)?,
                data: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(|error| error.to_string())
}

/// Test-only transaction with both project-revision and draft-revision CAS.
/// The project revision must already have been checked against native saved
/// project state by a future production caller; this isolated fixture only
/// verifies consistent conditional writes and recovery within its own table.
fn commit(
    db: &mut Connection,
    project_id: &str,
    expected_project_revision: u64,
    expected_draft_revision: Option<u64>,
    next: &str,
) -> Result<u64, String> {
    if project_id.is_empty()
        || project_id.len() > 120
        || !project_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || next.is_empty()
        || next.len() > MAX_DRAFT_BYTES
        || expected_project_revision > i64::MAX as u64
    {
        return Err("Invalid experimental crossfade draft input.".into());
    }
    let transaction = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let existing = read(&transaction, project_id)?;
    let result = match (existing, expected_draft_revision) {
        (None, None) => {
            transaction
                .execute(
                    "INSERT INTO crossfade_drafts_test
                     (project_id, project_revision, draft_revision, data)
                     VALUES (?1, ?2, 1, ?3)",
                    params![project_id, expected_project_revision, next],
                )
                .map_err(|error| error.to_string())?;
            1
        }
        (Some(old), Some(expected))
            if old.draft_revision == expected
                && old.project_revision == expected_project_revision =>
        {
            if old.data == next {
                return Ok(old.draft_revision);
            }
            let new_revision = old
                .draft_revision
                .checked_add(1)
                .ok_or("Crossfade draft revision overflow.")?;
            if new_revision > i64::MAX as u64 {
                return Err("Crossfade draft revision overflow.".into());
            }
            let changed = transaction
                .execute(
                    "UPDATE crossfade_drafts_test
                     SET draft_revision=?1, data=?2
                     WHERE project_id=?3 AND project_revision=?4 AND draft_revision=?5",
                    params![
                        new_revision,
                        next,
                        project_id,
                        expected_project_revision,
                        expected
                    ],
                )
                .map_err(|error| error.to_string())?;
            if changed != 1 {
                return Err("Crossfade draft changed elsewhere; reload.".into());
            }
            transaction
                .execute(
                    "INSERT INTO crossfade_draft_history_test
                     (project_id, draft_revision, data) VALUES (?1, ?2, ?3)",
                    params![project_id, old.draft_revision, old.data],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "DELETE FROM crossfade_draft_history_test
                     WHERE project_id=?1 AND id NOT IN (
                        SELECT id FROM crossfade_draft_history_test
                        WHERE project_id=?1 ORDER BY id DESC LIMIT 20
                     )",
                    params![project_id],
                )
                .map_err(|error| error.to_string())?;
            new_revision
        }
        _ => return Err("Crossfade draft changed elsewhere; reload.".into()),
    };
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        initialize(&connection).unwrap();
        connection
    }

    fn history(db: &Connection, project_id: &str) -> Vec<String> {
        let mut query = db
            .prepare(
                "SELECT data FROM crossfade_draft_history_test
                 WHERE project_id=?1 ORDER BY id",
            )
            .unwrap();
        query
            .query_map(params![project_id], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn inserts_new_draft_then_updates_with_undo_recovery_copy() {
        let mut connection = db();
        assert_eq!(
            commit(&mut connection, "project-1", 5, None, "draft-a"),
            Ok(1)
        );
        assert_eq!(
            commit(&mut connection, "project-1", 5, Some(1), "draft-b"),
            Ok(2)
        );
        assert_eq!(
            read(&connection, "project-1").unwrap().unwrap(),
            Stored {
                project_revision: 5,
                draft_revision: 2,
                data: "draft-b".into()
            }
        );
        assert_eq!(history(&connection, "project-1"), vec!["draft-a"]);
    }

    #[test]
    fn stale_draft_and_project_revisions_never_replace_live_bytes() {
        let mut connection = db();
        commit(&mut connection, "project-1", 5, None, "original").unwrap();
        for result in [
            commit(&mut connection, "project-1", 5, Some(0), "wrong"),
            commit(&mut connection, "project-1", 6, Some(1), "wrong"),
            commit(&mut connection, "project-1", 5, None, "wrong"),
        ] {
            assert!(result.is_err());
        }
        assert_eq!(
            read(&connection, "project-1").unwrap().unwrap().data,
            "original"
        );
        assert!(history(&connection, "project-1").is_empty());
    }

    #[test]
    fn unchanged_draft_does_not_increment_revision_or_create_history() {
        let mut connection = db();
        commit(&mut connection, "project-1", 5, None, "same").unwrap();
        assert_eq!(
            commit(&mut connection, "project-1", 5, Some(1), "same"),
            Ok(1)
        );
        assert!(history(&connection, "project-1").is_empty());
    }

    #[test]
    fn failed_history_write_rolls_back_draft_update() {
        let mut connection = db();
        commit(&mut connection, "project-1", 5, None, "untouched").unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER abort_draft_history BEFORE INSERT ON crossfade_draft_history_test
                 BEGIN SELECT RAISE(ABORT, 'simulated storage failure'); END;",
            )
            .unwrap();
        assert!(commit(&mut connection, "project-1", 5, Some(1), "new").is_err());
        assert_eq!(
            read(&connection, "project-1").unwrap().unwrap().data,
            "untouched"
        );
        assert_eq!(
            read(&connection, "project-1")
                .unwrap()
                .unwrap()
                .draft_revision,
            1
        );
    }

    #[test]
    fn retained_history_is_bounded_to_twenty_edits_per_project() {
        let mut connection = db();
        commit(&mut connection, "project-1", 1, None, "v1").unwrap();
        for revision in 1..=25 {
            commit(
                &mut connection,
                "project-1",
                1,
                Some(revision),
                &format!("v{}", revision + 1),
            )
            .unwrap();
        }
        let saved = history(&connection, "project-1");
        assert_eq!(saved.len(), 20);
        assert_eq!(saved.first().unwrap(), "v6");
        assert_eq!(saved.last().unwrap(), "v25");
        assert_eq!(
            read(&connection, "project-1")
                .unwrap()
                .unwrap()
                .draft_revision,
            26
        );
    }

    #[test]
    fn rejects_invalid_input_without_creating_state() {
        let mut connection = db();
        assert!(commit(&mut connection, "bad id", 0, None, "draft").is_err());
        assert!(commit(&mut connection, "p", 0, None, "").is_err());
        assert!(commit(
            &mut connection,
            "p",
            0,
            None,
            &"a".repeat(MAX_DRAFT_BYTES + 1)
        )
        .is_err());
        assert!(commit(&mut connection, "p", u64::MAX, None, "draft").is_err());
        assert!(read(&connection, "p").unwrap().is_none());
    }
}
