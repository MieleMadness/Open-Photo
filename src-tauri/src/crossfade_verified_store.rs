//! In-memory Phase J integration test: validate the entire draft snapshot
//! against the authoritative video sequence before one atomic CAS write.
//! This is NOT a production table, project migration, Tauri command or export.
use crate::crossfade_native_snapshot;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;

fn initialize() -> Connection {
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

/// Native-verified metadata must come from a trusted caller, independent of
/// the untrusted envelope. This fixture does not verify files on disk.
fn commit(
    db: &mut Connection,
    project_id: &str,
    expected_project_revision: u64,
    expected_draft_revision: Option<u64>,
    expected_source: &Value,
    fps: u64,
    clips: &[(&str, u64)],
    candidate: &str,
) -> Result<u64, String> {
    if !project_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || project_id.is_empty()
        || project_id.len() > 120
        || expected_project_revision > i64::MAX as u64
    {
        return Err("Invalid draft identity or project revision.".into());
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let authoritative: Option<u64> = tx
        .query_row(
            "SELECT revision FROM video_sequences WHERE id=?1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if authoritative != Some(expected_project_revision) {
        return Err("Video sequence changed; reopen its crossfade draft.".into());
    }
    crossfade_native_snapshot::validate_snapshot(
        candidate,
        expected_project_revision,
        expected_source,
        project_id,
        fps,
        clips,
    )?;
    let old: Option<(u64, u64, String)> = tx
        .query_row(
            "SELECT project_revision, revision, data FROM crossfade_drafts_test
             WHERE project_id=?1",
            params![project_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let next = match (old, expected_draft_revision) {
        (None, None) => {
            tx.execute(
                "INSERT INTO crossfade_drafts_test
                 (project_id, project_revision, revision, data) VALUES (?1, ?2, 1, ?3)",
                params![project_id, expected_project_revision, candidate],
            )
            .map_err(|error| error.to_string())?;
            1
        }
        (Some((bound, revision, previous)), Some(expected))
            if bound == expected_project_revision && revision == expected =>
        {
            if previous == candidate {
                return Ok(revision);
            }
            let advanced = revision
                .checked_add(1)
                .filter(|value| *value <= i64::MAX as u64)
                .ok_or("Draft revision overflow.")?;
            tx.execute(
                "INSERT INTO crossfade_draft_history_test
                 (project_id, revision, data) VALUES (?1, ?2, ?3)",
                params![project_id, revision, previous],
            )
            .map_err(|error| error.to_string())?;
            let updated = tx
                .execute(
                    "UPDATE crossfade_drafts_test SET revision=?1, data=?2
                     WHERE project_id=?3 AND project_revision=?4 AND revision=?5",
                    params![
                        advanced,
                        candidate,
                        project_id,
                        expected_project_revision,
                        expected
                    ],
                )
                .map_err(|error| error.to_string())?;
            if updated != 1 {
                return Err("Draft changed during the transaction.".into());
            }
            tx.execute(
                "DELETE FROM crossfade_draft_history_test
                 WHERE project_id=?1 AND id NOT IN (
                     SELECT id FROM crossfade_draft_history_test
                     WHERE project_id=?1 ORDER BY id DESC LIMIT 20
                 )",
                params![project_id],
            )
            .map_err(|error| error.to_string())?;
            advanced
        }
        _ => return Err("Draft changed elsewhere; reopen before saving.".into()),
    };
    tx.commit().map_err(|error| error.to_string())?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CLIPS: &[(&str, u64)] = &[("a", 60), ("b", 60)];

    fn source() -> Value {
        json!({"version":1,"clips":[
            {"clip_id":"a","asset_id":"asset-a","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1000,"source_in":{"ticks":"0","time_base":"1/1000000"},"source_out":{"ticks":"2000000","time_base":"1/1000000"}},
            {"clip_id":"b","asset_id":"asset-b","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","bytes":2000,"source_in":{"ticks":"0","time_base":"1/1000000"},"source_out":{"ticks":"2000000","time_base":"1/1000000"}}
        ]})
    }

    fn snapshot(source: &Value, fade_frames: u64) -> String {
        let draft = json!({
            "version":1,"project_id":"project-1","fps":30,
            "clips":[{"id":"a","frames":60},{"id":"b","frames":60}],
            "boundaries":[{"from_clip_id":"a","to_clip_id":"b","frames":fade_frames}]
        })
        .to_string();
        json!({
            "version":1,"project_revision":5,"source_identity":source,"draft":draft
        })
        .to_string()
    }

    fn db() -> Connection {
        let db = initialize();
        db.execute(
            "INSERT INTO video_sequences (id,revision,data) VALUES ('project-1',5,'{}')",
            [],
        )
        .unwrap();
        db
    }

    fn stored(db: &Connection) -> Option<(u64, String)> {
        db.query_row(
            "SELECT revision,data FROM crossfade_drafts_test WHERE project_id='project-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .unwrap()
    }

    fn save(
        db: &mut Connection,
        revision: u64,
        draft_revision: Option<u64>,
        manifest: &Value,
        candidate: &str,
    ) -> Result<u64, String> {
        commit(
            db,
            "project-1",
            revision,
            draft_revision,
            manifest,
            30,
            CLIPS,
            candidate,
        )
    }

    #[test]
    fn validates_and_commits_two_versions_with_recovery_history() {
        let mut db = db();
        let first = snapshot(&source(), 12);
        let second = snapshot(&source(), 15);
        assert_eq!(save(&mut db, 5, None, &source(), &first), Ok(1));
        assert_eq!(save(&mut db, 5, Some(1), &source(), &second), Ok(2));
        assert_eq!(stored(&db), Some((2, second)));
        let recovered: String = db
            .query_row(
                "SELECT data FROM crossfade_draft_history_test WHERE project_id='project-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(recovered, first);
    }

    #[test]
    fn rejects_malformed_and_untrusted_source_before_first_write() {
        let mut db = db();
        let good = snapshot(&source(), 12);
        assert!(save(&mut db, 5, None, &source(), "{}").is_err());
        let mut changed = source();
        changed["clips"][0]["sha256"] =
            json!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc");
        assert!(save(&mut db, 5, None, &changed, &good).is_err());
        assert!(stored(&db).is_none());
    }

    #[test]
    fn rejects_authoritative_project_revision_changes_without_overwrite() {
        let mut db = db();
        let first = snapshot(&source(), 12);
        assert_eq!(save(&mut db, 5, None, &source(), &first), Ok(1));
        db.execute(
            "UPDATE video_sequences SET revision=6 WHERE id='project-1'",
            [],
        )
        .unwrap();
        assert!(save(&mut db, 5, Some(1), &source(), &snapshot(&source(), 15)).is_err());
        assert!(save(&mut db, 6, Some(1), &source(), &snapshot(&source(), 15)).is_err());
        assert_eq!(stored(&db), Some((1, first)));
    }

    #[test]
    fn invalid_transition_and_stale_draft_revision_leave_bytes_unchanged() {
        let mut db = db();
        let first = snapshot(&source(), 12);
        assert_eq!(save(&mut db, 5, None, &source(), &first), Ok(1));
        assert!(save(&mut db, 5, Some(1), &source(), &snapshot(&source(), 60)).is_err());
        assert!(save(&mut db, 5, Some(0), &source(), &snapshot(&source(), 15)).is_err());
        assert_eq!(stored(&db), Some((1, first)));
    }

    #[test]
    fn verified_updates_keep_only_twenty_recoverable_drafts() {
        let mut db = db();
        let first = snapshot(&source(), 12);
        assert_eq!(save(&mut db, 5, None, &source(), &first), Ok(1));
        for revision in 1..=25 {
            let candidate = snapshot(&source(), if revision % 2 == 0 { 12 } else { 15 });
            assert_eq!(
                save(&mut db, 5, Some(revision), &source(), &candidate),
                Ok(revision + 1)
            );
        }
        let (count, earliest, latest): (u64, u64, u64) = db
            .query_row(
                "SELECT COUNT(*), MIN(revision), MAX(revision)
                 FROM crossfade_draft_history_test WHERE project_id='project-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!((count, earliest, latest), (20, 6, 25));
        assert_eq!(stored(&db).unwrap().0, 26);
    }

    #[test]
    fn failed_recovery_write_aborts_entire_validated_update() {
        let mut db = db();
        let first = snapshot(&source(), 12);
        assert_eq!(save(&mut db, 5, None, &source(), &first), Ok(1));
        db.execute_batch(
            "CREATE TRIGGER abort_history BEFORE INSERT ON crossfade_draft_history_test
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
        )
        .unwrap();
        assert!(save(&mut db, 5, Some(1), &source(), &snapshot(&source(), 15)).is_err());
        assert_eq!(stored(&db), Some((1, first)));
    }
}
