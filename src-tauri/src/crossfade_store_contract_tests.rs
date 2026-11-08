fn seeded() -> (Fixture, Context, String, String) {
    let mut f = Fixture::new();
    let c = f.context();
    let first = f.snapshot(&c, 6);
    let second = f.snapshot(&c, 9);
    commit(&mut f.db, &c, None, &first, |_| Ok(())).unwrap();
    commit(&mut f.db, &c, Some(1), &second, |_| Ok(())).unwrap();
    (f, c, first, second)
}
fn copies(f: &Fixture) -> i64 {
    f.db.query_row(
        "SELECT count(*) FROM phase_j_sequence_recovery_test",
        [],
        |r| r.get(0),
    )
    .unwrap()
}
#[test]
fn storage_version_is_component_owned_and_initialization_is_idempotent() {
    let (mut f, c, _, second) = seeded();
    f.db.execute_batch("PRAGMA user_version=42").unwrap();
    let raw = f.raw_project();
    initialize_detached(&f.db).unwrap();
    initialize_detached(&f.db).unwrap();
    assert_eq!(
        f.db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        42
    );
    assert_eq!(f.raw_project(), raw);
    assert_eq!(
        load(&mut f.db, &c.root, &c.project.id)
            .unwrap()
            .unwrap()
            .data,
        second
    );
    let choice = prepare_recovery_copy(&mut f.db, &c.root, &c.project.id, 1).unwrap();
    recover_copy(&mut f.db, &choice, |_| Ok(())).unwrap();
    // The existing client can still edit a hard-cut project, but its edit makes
    // every old draft/history/copy stale. Nothing is silently rebased.
    initialize(&f.db).unwrap();
    store(&f.db, &c.root, &f.project, Some(1)).unwrap();
    assert!(load(&mut f.db, &c.root, &c.project.id).is_err());
    assert!(prepare_recovery_copy(&mut f.db, &c.root, &c.project.id, 1).is_err());
    assert!(read_recovery_copy(&mut f.db, &c.root, &c.project.id, &choice.copy_id).is_err());
    assert_eq!((f.history(), copies(&f)), (1, 1));
    let mut future = serde_json::to_value(&f.project).unwrap();
    future["schema_version"] = json!(4);
    assert!(parse(future.to_string().as_bytes()).is_err());
}
#[test]
fn future_unversioned_and_partial_storage_are_preserved_and_refused() {
    let (mut f, c, _, second) = seeded();
    f.db.execute("UPDATE phase_j_sequence_schema_test SET version=2", [])
        .unwrap();
    assert!(initialize_detached(&f.db).is_err());
    assert!(load(&mut f.db, &c.root, &c.project.id).is_err());
    assert!(commit(&mut f.db, &c, Some(2), &second, |_| Ok(())).is_err());
    assert!(prepare_recovery_copy(&mut f.db, &c.root, &c.project.id, 1).is_err());
    assert_eq!(stored(&f.db, &c).unwrap().unwrap().data, second);
    f.db.execute_batch("DROP TABLE phase_j_sequence_schema_test")
        .unwrap();
    assert!(initialize_detached(&f.db).is_err());
    assert_eq!(stored(&f.db, &c).unwrap().unwrap().data, second);
    assert_eq!(f.history(), 1);
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE phase_j_sequence_schema_test(singleton INTEGER,version INTEGER); INSERT INTO phase_j_sequence_schema_test VALUES(1,1);").unwrap();
    assert!(initialize_detached(&db).is_err());
    assert!(!table_exists(&db, "phase_j_sequence_drafts_test").unwrap());
    let db = Connection::open_in_memory().unwrap();
    initialize_detached(&db).unwrap();
    db.execute_batch("DROP TABLE phase_j_sequence_history_test; CREATE TABLE phase_j_sequence_history_test(root TEXT NOT NULL,project_id TEXT NOT NULL,project_revision INTEGER NOT NULL,project_hash TEXT NOT NULL,revision INTEGER NOT NULL,data TEXT NOT NULL,data_hash TEXT NOT NULL);").unwrap();
    // Matching names alone are insufficient: losing uniqueness permits
    // ambiguous revision selection. Initialization must not repair silently.
    assert!(initialize_detached(&db).is_err());
}
#[test]
fn recovery_is_an_explicit_scoped_copy_and_retains_current_and_history() {
    let (mut f, c, first, second) = seeded();
    let raw = f.raw_project();
    let choice = prepare_recovery_copy(&mut f.db, &c.root, &c.project.id, 1).unwrap();
    assert_eq!(copies(&f), 0);
    assert_eq!(
        recover_copy(&mut f.db, &choice, |_| Ok(())).unwrap(),
        choice.copy_id
    );
    f.reopen();
    assert_eq!(
        read_recovery_copy(&mut f.db, &c.root, &c.project.id, &choice.copy_id)
            .unwrap()
            .data,
        first
    );
    assert_eq!(
        load(&mut f.db, &c.root, &c.project.id)
            .unwrap()
            .unwrap()
            .data,
        second
    );
    assert_eq!(f.raw_project(), raw);
    assert_eq!((f.history(), copies(&f)), (1, 1));
    let mut wrong = choice.clone();
    wrong.history_revision = 2;
    assert!(recover_copy(&mut f.db, &wrong, |_| Ok(())).is_err());
    wrong = choice.clone();
    wrong.project_id = "another-project".into();
    assert!(recover_copy(&mut f.db, &wrong, |_| Ok(())).is_err());
    let other = Fixture::new();
    wrong = choice.clone();
    wrong.root = other.root.to_str().unwrap().into();
    assert!(recover_copy(&mut f.db, &wrong, |_| Ok(())).is_err());
    assert_eq!(copies(&f), 1);
}
#[test]
fn corrupt_head_is_preserved_while_verified_history_can_be_copied() {
    let (mut f, c, first, _) = seeded();
    let corrupt = "{corrupt current draft";
    f.db.execute("UPDATE phase_j_sequence_drafts_test SET data=?1", [corrupt])
        .unwrap();
    assert!(load(&mut f.db, &c.root, &c.project.id).is_err());
    assert!(commit(&mut f.db, &c, Some(2), &first, |_| Ok(())).is_err());
    let choice = prepare_recovery_copy(&mut f.db, &c.root, &c.project.id, 1).unwrap();
    recover_copy(&mut f.db, &choice, |_| Ok(())).unwrap();
    assert_eq!(stored(&f.db, &c).unwrap().unwrap().data, corrupt);
    assert_eq!(
        read_recovery_copy(&mut f.db, &c.root, &c.project.id, &choice.copy_id)
            .unwrap()
            .data,
        first
    );
    f.db.execute(
        "UPDATE phase_j_sequence_recovery_test SET data=?1",
        [f.snapshot(&c, 12)],
    )
    .unwrap();
    assert!(read_recovery_copy(&mut f.db, &c.root, &c.project.id, &choice.copy_id).is_err());
    assert!(recover_copy(&mut f.db, &choice, |_| Ok(())).is_err());
}
#[test]
fn selections_detect_changes_and_committed_retries_are_idempotent() {
    let (mut f, c, first, second) = seeded();
    let stale = prepare_recovery_copy(&mut f.db, &c.root, &c.project.id, 1).unwrap();
    commit(&mut f.db, &c, Some(2), &first, |_| Ok(())).unwrap();
    assert!(recover_copy(&mut f.db, &stale, |_| Ok(())).is_err());
    assert_eq!(copies(&f), 0);
    let choice = prepare_recovery_copy(&mut f.db, &c.root, &c.project.id, 1).unwrap();
    let result = recover_copy(&mut f.db, &choice, |at| {
        if at == "after_copy_commit" {
            Err("lost reply".into())
        } else {
            Ok(())
        }
    });
    assert!(result.is_err());
    f.reopen();
    assert_eq!(copies(&f), 1);
    commit(&mut f.db, &c, Some(3), &second, |_| Ok(())).unwrap();
    for _ in 0..3 {
        assert_eq!(
            recover_copy(&mut f.db, &choice, |_| panic!("retry must not write")).unwrap(),
            choice.copy_id
        );
    }
    assert_eq!(copies(&f), 1);
    let mut conflict = choice.clone();
    conflict.head_hash = "different request".into();
    assert!(recover_copy(&mut f.db, &conflict, |_| Ok(())).is_err());
}
#[test]
fn copy_faults_roll_back_and_changed_selected_history_is_refused() {
    for failure in ["after_copy", "before_copy_commit"] {
        let (mut f, c, first, second) = seeded();
        let choice = prepare_recovery_copy(&mut f.db, &c.root, &c.project.id, 1).unwrap();
        assert!(recover_copy(&mut f.db, &choice, |at| if at == failure {
            Err("injected".into())
        } else {
            Ok(())
        })
        .is_err());
        f.reopen();
        f.reopen();
        assert_eq!((copies(&f), f.history()), (0, 1));
        assert_eq!(
            load(&mut f.db, &c.root, &c.project.id)
                .unwrap()
                .unwrap()
                .data,
            second
        );
        assert_eq!(history_draft(&f.db, &c, 1).unwrap().data, first);
        // Valid replacement with a valid checksum is still a different selection.
        f.db.execute(
            "UPDATE phase_j_sequence_history_test SET data=?1,data_hash=?2",
            params![second, payload_hash(&second)],
        )
        .unwrap();
        assert!(recover_copy(&mut f.db, &choice, |_| Ok(())).is_err());
    }
}
#[test]
fn history_is_bounded_append_only_and_isolates_corrupt_entries() {
    let (mut f, c, first, second) = seeded();
    for revision in 2..32 {
        let next = if revision % 2 == 0 { &first } else { &second };
        commit(&mut f.db, &c, Some(revision), next, |_| Ok(())).unwrap();
    }
    assert_eq!(f.history(), 31);
    f.db.execute(
        "UPDATE phase_j_sequence_history_test SET data=?1 WHERE revision=30",
        [&first],
    )
    .unwrap();
    let first_page = history_page(&mut f.db, &c.root, &c.project.id, None).unwrap();
    assert_eq!(first_page.entries.len(), 25);
    assert_eq!(
        first_page
            .entries
            .iter()
            .filter(|e| e.error.is_some())
            .map(|e| e.revision)
            .collect::<Vec<_>>(),
        vec![30]
    );
    let last_page = history_page(&mut f.db, &c.root, &c.project.id, first_page.next).unwrap();
    assert_eq!(last_page.entries.len(), 6);
    assert_eq!(last_page.next, None);
    assert!(last_page.entries.iter().all(|e| e.error.is_none()));
    assert!(history_page(&mut f.db, &c.root, &c.project.id, Some(0)).is_err());
    assert!(prepare_recovery_copy(&mut f.db, &c.root, &c.project.id, 30).is_err());
    assert_eq!(f.history(), 31);
}

// These children only open the parent's disposable fixture. Abrupt exit skips
// Rust destructors and tests SQLite crash recovery, not hardware power loss.
#[test]
fn storage_contract_crash_child() {
    let Ok(root) = std::env::var("OPEN_PHOTO_PHASE_J_CRASH_ROOT") else {
        return;
    };
    let id = std::env::var("OPEN_PHOTO_PHASE_J_CRASH_ID").unwrap();
    let at = std::env::var("OPEN_PHOTO_PHASE_J_CRASH_AT").unwrap();
    let mut db = Connection::open(PathBuf::from(&root).join("prototype.db")).unwrap();
    let mut stop = |name: &str| -> Result<(), String> {
        if name == at {
            std::process::exit(77);
        }
        Ok(())
    };
    if let Ok(choice) = std::env::var("OPEN_PHOTO_PHASE_J_CRASH_CHOICE") {
        recover_copy(&mut db, &serde_json::from_str(&choice).unwrap(), &mut stop).unwrap();
    } else {
        let c = capture(&mut db, &root, &id).unwrap();
        let next = c.encode(&[(&c.clips[0].0, &c.clips[1].0, 12)]).unwrap();
        commit(&mut db, &c, Some(2), &next, &mut stop).unwrap();
    }
    panic!("checkpoint was not reached");
}
#[test]
fn abrupt_process_exit_recovers_draft_and_copy_transactions() {
    for at in [
        "after_history",
        "after_draft",
        "before_commit",
        "after_commit",
        "after_copy",
        "before_copy_commit",
        "after_copy_commit",
    ] {
        let (mut f, c, _, second) = seeded();
        let choice = prepare_recovery_copy(&mut f.db, &c.root, &c.project.id, 1).unwrap();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "video_sequence::phase_j_sequence_binding::tests::storage_contract_crash_child",
                "--nocapture",
            ])
            .env("OPEN_PHOTO_PHASE_J_CRASH_ROOT", &f.root)
            .env("OPEN_PHOTO_PHASE_J_CRASH_ID", &c.project.id)
            .env("OPEN_PHOTO_PHASE_J_CRASH_AT", at)
            .env_remove("OPEN_PHOTO_PHASE_J_CRASH_CHOICE");
        if at.contains("copy") {
            command.env(
                "OPEN_PHOTO_PHASE_J_CRASH_CHOICE",
                serde_json::to_string(&choice).unwrap(),
            );
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(77),
            "{at}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        f.reopen();
        f.reopen();
        let head = load(&mut f.db, &c.root, &c.project.id).unwrap().unwrap();
        if at == "after_commit" {
            assert_eq!(head.revision, 3);
            assert_eq!(f.history(), 2);
        } else {
            assert_eq!(head.revision, 2);
            assert_eq!(head.data, second);
            assert_eq!(f.history(), 1);
        }
        assert_eq!(copies(&f), i64::from(at == "after_copy_commit"));
        if at == "after_copy_commit" {
            recover_copy(&mut f.db, &choice, |_| panic!("retry write")).unwrap();
            assert_eq!(copies(&f), 1);
        }
    }
}
