#[cfg(test)]
#[cfg(windows)]
mod intent_binding_tests {
    use super::step2_apply_tests::Fixture;
    use super::*;
    const CHANGES: &[(&str, &str)] = &[
        ("source_hash", "changed-source-checksum"),
        ("source_identity", "changed-source-identity"),
        ("root_identity", "changed-root-identity"),
        ("path", "changed-photo-path"),
        ("root", "changed-root-path"),
        ("identity", "changed-sidecar-identity"),
        ("original", "changed-original"),
        ("original_hash", "changed-original-checksum"),
        ("candidate", "changed-candidate"),
        ("candidate_hash", "changed-candidate-checksum"),
        ("decision_hash", "changed-approval"),
        ("state", "cancelled"),
        ("backup_name", "changed-backup-name"),
    ];
    fn change(database: &Path, id: i64, column: &str, value: &str) {
        assert!(CHANGES.iter().any(|(allowed, _)| *allowed == column));
        let db = Connection::open(database).unwrap();
        db.execute(
            &format!("UPDATE xmp_operations SET {column}=?1 WHERE id=?2"),
            params![value, id],
        )
        .unwrap();
    }

    #[test]
    fn apply_refuses_any_changed_intent_binding_before_moving_the_original() {
        for (column, value) in CHANGES {
            let mut f = Fixture::new();
            let (row, view) = f.proposal();
            let original = fs::read(f.sidecar()).unwrap();
            let database = f.root.join("user-data.sqlite");
            let result = apply_on(
                &mut f.db,
                "w",
                row.id,
                &view.fingerprint,
                &row.candidate_hash,
                |point, _| {
                    if point == "after_apply_intent_commit" {
                        change(&database, row.id, column, value);
                    }
                    Ok(())
                },
            );
            assert!(result.is_err(), "Apply accepted changed {column}");
            assert_eq!(fs::read(f.sidecar()).unwrap(), original, "changed {column}");
            assert_eq!(fs::read(&f.source).unwrap(), b"original-media");
            assert!(!f
                .root
                .join(format!(
                    ".openphoto-xmp-original-{}-{}.bak",
                    row.id,
                    std::process::id()
                ))
                .exists());
        }
    }

    #[test]
    fn restore_refuses_any_changed_intent_binding_without_consuming_the_backup() {
        for (column, value) in CHANGES {
            let mut f = Fixture::new();
            let (row, view) = f.proposal();
            apply_on(
                &mut f.db,
                "w",
                row.id,
                &view.fingerprint,
                &row.candidate_hash,
                |point, path| {
                    if point == "after_original_move" {
                        fs::write(path, b"disposable-competing-file").unwrap();
                        return Err("fixture interruption".into());
                    }
                    Ok(())
                },
            )
            .unwrap_err();
            fs::remove_file(f.sidecar()).unwrap();
            let ready = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
            assert!(ready.can_restore);
            let backup = f.root.join(ready.backup_name.as_ref().unwrap());
            let original = fs::read(&backup).unwrap();
            let database = f.root.join("user-data.sqlite");
            let result = restore_on(
                &mut f.db,
                "w",
                row.id,
                &ready.fingerprint,
                &row.candidate_hash,
                |point, _| {
                    if point == "after_restore_intent_commit" {
                        change(&database, row.id, column, value);
                    }
                    Ok(())
                },
            );
            assert!(result.is_err(), "Restore accepted changed {column}");
            assert!(!f.sidecar().exists(), "changed {column}");
            assert_eq!(fs::read(&backup).unwrap(), original, "changed {column}");
            assert_eq!(fs::read(&f.source).unwrap(), b"original-media");
        }
    }

    #[test]
    fn new_sidecar_refuses_changed_prepared_records_before_publication() {
        for (column, value) in CHANGES {
            let mut f = Fixture::new();
            fs::remove_file(f.sidecar()).unwrap();
            let database = f.root.join("user-data.sqlite");
            let result = batch_with_checkpoint(
                &mut f.db,
                "w".into(),
                f.root.to_str().unwrap().into(),
                vec![f.source.to_str().unwrap().into()],
                vec!["New".into()],
                |point, _| {
                    if point == "after_record_commit" {
                        let db = Connection::open(&database).unwrap();
                        let id = db
                            .query_row(
                                "SELECT id FROM xmp_operations WHERE workspace='w'",
                                [],
                                |r| r.get::<_, i64>(0),
                            )
                            .unwrap();
                        drop(db);
                        change(&database, id, column, value);
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(result.created, 0, "New sidecar accepted changed {column}");
            assert!(!result.errors.is_empty(), "changed {column}");
            assert!(!f.sidecar().exists(), "changed {column}");
            assert_eq!(fs::read(&f.source).unwrap(), b"original-media");
        }
    }

    #[test]
    fn new_sidecar_rechecks_registered_folder_binding_after_record_commit() {
        let mut f = Fixture::new();
        fs::remove_file(f.sidecar()).unwrap();
        let database = f.root.join("user-data.sqlite");
        let result=batch_with_checkpoint(&mut f.db,"w".into(),f.root.to_str().unwrap().into(),vec![f.source.to_str().unwrap().into()],vec!["New".into()],|point,_| {
            if point=="after_record_commit" {
                let db=Connection::open(&database).unwrap();
                db.execute("UPDATE xmp_folder_bindings SET identity='changed-binding' WHERE workspace='w'",[]).unwrap();
            }
            Ok(())
        }).unwrap();
        assert_eq!(result.created, 0);
        assert!(!result.errors.is_empty());
        assert!(!f.sidecar().exists());
    }
}
