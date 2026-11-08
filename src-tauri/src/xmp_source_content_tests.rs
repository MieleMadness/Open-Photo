#[cfg(test)]
#[cfg(windows)]
mod source_content_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    const ORIGINAL: &[u8] = b"original-media";
    const CHANGED: &[u8] = b"modified-media";

    struct Fixture {
        root: PathBuf,
        source: PathBuf,
        db: Connection,
    }
    impl Fixture {
        fn new() -> Self {
            let root = loop {
                let path = std::env::temp_dir().join(format!(
                    "openphoto-source-check-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => break path,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(e) => panic!("Cannot create disposable fixture: {e}"),
                }
            };
            let source = root.join("photo.jpg");
            fs::write(&source, ORIGINAL).unwrap();
            fs::write(
                root.join("photo.jpg.xmp"),
                crate::render_openphoto_xmp(&["Old".into()]),
            )
            .unwrap();
            let db = Connection::open(root.join("journal.sqlite")).unwrap();
            schema(&db).unwrap();
            db.execute_batch("CREATE TABLE user_data(key TEXT PRIMARY KEY,value TEXT NOT NULL)")
                .unwrap();
            db.execute(
                "INSERT INTO user_data VALUES('openphoto.workspaces',?1)",
                [serde_json::json!([{"id":"w","roots":[root.to_str().unwrap()]}]).to_string()],
            )
            .unwrap();
            Self { root, source, db }
        }
        fn sidecar(&self) -> PathBuf {
            self.root.join("photo.jpg.xmp")
        }
        fn propose(&mut self) -> (Review, Snapshot) {
            let result = batch(
                &mut self.db,
                "w".into(),
                self.root.to_str().unwrap().into(),
                vec![self.source.to_str().unwrap().into()],
                vec!["New".into()],
            )
            .unwrap();
            assert!(result.errors.is_empty(), "{:?}", result.errors);
            assert_eq!(result.review, 1);
            let row = list(&self.db, "w").unwrap().remove(0);
            let view = review_snapshot_on(&mut self.db, "w", row.id).unwrap();
            assert!(view.can_apply, "{:?}", view.reason);
            (row, view)
        }
        fn apply(&mut self, row: &Review, view: &Snapshot) -> Result<Applied, String> {
            apply_on(
                &mut self.db,
                "w",
                row.id,
                &view.fingerprint,
                &row.candidate_hash,
                |_, _| Ok(()),
            )
        }
        fn edit_same_file_and_restore_timestamp(&self) {
            let before = fs::metadata(&self.source).unwrap();
            let file = fs::OpenOptions::new()
                .write(true)
                .open(&self.source)
                .unwrap();
            (&file).write_all(CHANGED).unwrap();
            file.set_times(fs::FileTimes::new().set_modified(before.modified().unwrap()))
                .unwrap();
            drop(file);
            let after = fs::metadata(&self.source).unwrap();
            assert_eq!(before.len(), after.len());
            assert_eq!(before.modified().unwrap(), after.modified().unwrap());
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.db = Connection::open_in_memory().unwrap();
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn same_identity_length_and_timestamp_edit_blocks_stale_and_fresh_apply_after_restart() {
        let mut f = Fixture::new();
        let (row, view) = f.propose();
        assert_eq!(row.source_hash.as_deref(), Some(hash(ORIGINAL).as_str()));
        let sidecar = fs::read(f.sidecar()).unwrap();
        f.edit_same_file_and_restore_timestamp();
        f.db = Connection::open(f.root.join("journal.sqlite")).unwrap();
        schema(&f.db).unwrap();
        let guard = Guard::open(f.root.to_str().unwrap(), f.source.to_str().unwrap()).unwrap();
        assert_eq!(
            row.source_identity.as_deref(),
            Some(guard.source_identity.as_str())
        );
        drop(guard);
        assert!(f
            .apply(&row, &view)
            .unwrap_err()
            .contains("contents changed"));
        let fresh = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(!fresh.can_apply);
        assert_ne!(view.fingerprint, fresh.fingerprint);
        assert!(f.apply(&row, &fresh).is_err());
        assert_eq!(fs::read(f.sidecar()).unwrap(), sidecar);
        assert_eq!(fs::read(&f.source).unwrap(), CHANGED);
        assert_eq!(
            load_review(&f.db, "w", row.id).unwrap().state,
            "review_required"
        );
        review_decide_on(&mut f.db, "w", row.id, "cancelled", &row.candidate_hash).unwrap();
        let (new, new_view) = f.propose();
        assert_eq!(new.source_hash, Some(hash(CHANGED)));
        f.apply(&new, &new_view).unwrap();
        assert_eq!(fs::read(&f.source).unwrap(), CHANGED);
    }

    #[test]
    fn source_change_before_first_comparison_is_not_silently_rebound() {
        let mut f = Fixture::new();
        let result = batch(
            &mut f.db,
            "w".into(),
            f.root.to_str().unwrap().into(),
            vec![f.source.to_str().unwrap().into()],
            vec!["New".into()],
        )
        .unwrap();
        assert_eq!(result.review, 1);
        assert!(result.errors.is_empty());
        let row = list(&f.db, "w").unwrap().remove(0);
        f.edit_same_file_and_restore_timestamp();
        let view = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(!view.can_apply);
        assert_eq!(
            load_review(&f.db, "w", row.id).unwrap().source_hash,
            Some(hash(ORIGINAL))
        );
    }

    #[test]
    fn legacy_schema_migrates_idempotently_without_blessing_existing_proposals() {
        let mut f = Fixture::new();
        let (row, _) = f.propose();
        f.db.execute_batch("ALTER TABLE xmp_operations DROP COLUMN source_hash")
            .unwrap();
        schema(&f.db).unwrap();
        schema(&f.db).unwrap();
        assert!(load_review(&f.db, "w", row.id)
            .unwrap()
            .source_hash
            .is_none());
        let view = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(!view.can_apply);
        assert!(view
            .reason
            .as_deref()
            .unwrap()
            .contains("lacks a source checksum"));
        review_decide_on(&mut f.db, "w", row.id, "cancelled", &row.candidate_hash).unwrap();
        assert_eq!(fs::read(&f.source).unwrap(), ORIGINAL);
    }

    #[test]
    fn changed_source_blocks_restoration_without_touching_saved_original() {
        let mut f = Fixture::new();
        let (row, view) = f.propose();
        // Simulate a persisted interruption with a competing destination so
        // handled cleanup must leave the protected original in its backup.
        apply_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, path| {
                if point == "after_original_move" {
                    fs::write(path, b"competing-sidecar").unwrap();
                    return Err("fixture stop".into());
                }
                Ok(())
            },
        )
        .unwrap_err();
        fs::remove_file(f.sidecar()).unwrap();
        let before = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(before.can_restore);
        let backup = f.root.join(before.backup_name.as_ref().unwrap());
        let bytes = fs::read(&backup).unwrap();
        f.edit_same_file_and_restore_timestamp();
        let fresh = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(!fresh.can_restore);
        assert!(restore_on(
            &mut f.db,
            "w",
            row.id,
            &before.fingerprint,
            &row.candidate_hash,
            |_, _| Ok(())
        )
        .is_err());
        assert!(!f.sidecar().exists());
        assert_eq!(fs::read(backup).unwrap(), bytes);
        assert_eq!(fs::read(&f.source).unwrap(), CHANGED);
    }

    #[test]
    fn streamed_hash_covers_bytes_beyond_first_buffer_and_rewinds() {
        let f = Fixture::new();
        let mut bytes = vec![42u8; 3 * 64 * 1024 + 7];
        fs::write(&f.source, &bytes).unwrap();
        let mut guard = Guard::open(f.root.to_str().unwrap(), f.source.to_str().unwrap()).unwrap();
        assert_eq!(guard.source_hash().unwrap(), hash(&bytes));
        assert_eq!(guard.source_hash().unwrap(), hash(&bytes));
        drop(guard);
        bytes[2 * 64 * 1024 + 1] = 43;
        fs::write(&f.source, &bytes).unwrap();
        let mut guard = Guard::open(f.root.to_str().unwrap(), f.source.to_str().unwrap()).unwrap();
        assert_eq!(guard.source_hash().unwrap(), hash(&bytes));
        assert_ne!(guard.source_hash().unwrap(), hash(&vec![42u8; bytes.len()]));
    }

    #[test]
    fn source_remains_write_protected_after_checksum_until_publication() {
        let mut f = Fixture::new();
        let (row, view) = f.propose();
        let source = f.source.clone();
        let mut attempts = 0;
        apply_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, _| {
                if matches!(
                    point,
                    "before_original_move" | "after_original_move" | "after_apply_publish"
                ) {
                    let error = fs::OpenOptions::new()
                        .write(true)
                        .open(&source)
                        .unwrap_err();
                    assert_eq!(error.raw_os_error(), Some(32), "{error}");
                    attempts += 1;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(attempts, 3);
        assert_eq!(fs::read(&f.source).unwrap(), ORIGINAL);
    }
}
