fn checkpoint_fixture() -> (Fixture, EditorSession) {
    let (f, s) = session_fixture();
    initialize_checkpoint_store(&f.db).unwrap();
    (f, s)
}
fn checkpoint_count(f: &Fixture) -> i64 {
    f.db.query_row(
        "SELECT count(*) FROM phase_j_session_checkpoints_test",
        [],
        |r| r.get(0),
    )
    .unwrap()
}
#[test]
fn checkpoint_roundtrip_restores_selection_revision_and_both_history_stacks() {
    let (mut f, mut s) = checkpoint_fixture();
    let raw = f.raw_project();
    f.db.execute_batch("PRAGMA user_version=42").unwrap();
    initialize_checkpoint_store(&f.db).unwrap();
    let first = save_checkpoint(&mut f.db, &s, None, |_| Ok(())).unwrap();
    session_edit(&mut f, &mut s, Some(18)).unwrap();
    session_edit(&mut f, &mut s, Some(21)).unwrap();
    s.apply(&mut f.db, &s.receipt.clone(), SessionAction::Undo, |_| {
        Ok(())
    })
    .unwrap();
    let latest = save_checkpoint(&mut f.db, &s, Some(&first), |_| Ok(())).unwrap();
    f.reopen();
    f.reopen();
    let mut resumed = resume_checkpoint(&mut f.db, &s.root, &s.project_id, &latest).unwrap();
    assert_eq!(resumed, s);
    resumed
        .apply(
            &mut f.db,
            &resumed.receipt.clone(),
            SessionAction::Redo,
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(resumed.receipt.revision, s.receipt.revision + 1);
    assert_eq!(checkpoint_count(&f), 2);
    assert_eq!(f.raw_project(), raw);
    assert_eq!(
        f.db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        42
    );
}
#[test]
fn checkpoint_conflicts_across_connections_preserve_both_working_sessions() {
    let (mut f, mut a) = checkpoint_fixture();
    let base = save_checkpoint(&mut f.db, &a, None, |_| Ok(())).unwrap();
    let mut other = Connection::open(f.root.join("prototype.db")).unwrap();
    let mut b = resume_checkpoint(&mut other, &a.root, &a.project_id, &base).unwrap();
    session_edit(&mut f, &mut a, Some(18)).unwrap();
    session_edit(&mut f, &mut b, Some(21)).unwrap();
    let latest = save_checkpoint(&mut f.db, &a, Some(&base), |_| Ok(())).unwrap();
    let before = b.clone();
    assert!(save_checkpoint(&mut other, &b, Some(&base), |_| Ok(())).is_err());
    assert_eq!(b, before);
    assert_eq!(
        resume_checkpoint(&mut other, &a.root, &a.project_id, &latest).unwrap(),
        a
    );
    assert!(resume_checkpoint(&mut other, &a.root, &a.project_id, &base).is_err());
    assert_eq!(
        save_checkpoint(
            &mut other,
            &resume_checkpoint(&mut f.db, &a.root, &a.project_id, &latest).unwrap(),
            Some(&base),
            |_| Ok(())
        )
        .unwrap(),
        latest
    );
    assert_eq!(checkpoint_count(&f), 2);
}
#[test]
fn recovery_forks_a_reviewed_historical_checkpoint_once_and_keeps_original_head() {
    let (mut f, mut s) = checkpoint_fixture();
    let first = save_checkpoint(&mut f.db, &s, None, |_| Ok(())).unwrap();
    session_edit(&mut f, &mut s, Some(18)).unwrap();
    let latest = save_checkpoint(&mut f.db, &s, Some(&first), |_| Ok(())).unwrap();
    let choice = prepare_checkpoint_recovery(&mut f.db, &s.root, &s.project_id, &first).unwrap();
    assert_eq!(checkpoint_count(&f), 2);
    assert!(
        recover_checkpoint(&mut f.db, &s.root, &s.project_id, &choice, |at| {
            if at == "after_recovery_commit" {
                Err("lost reply".into())
            } else {
                Ok(())
            }
        })
        .is_err()
    );
    f.reopen();
    let recovered =
        recover_checkpoint(&mut f.db, &s.root, &s.project_id, &choice, |_| Ok(())).unwrap();
    let copy = resume_checkpoint(&mut f.db, &s.root, &s.project_id, &recovered).unwrap();
    assert_eq!(copy.recovered_from, Some(first.clone()));
    assert_eq!(copy.receipt.revision, 1);
    assert_ne!(copy.receipt.session_id, s.receipt.session_id);
    assert_eq!(
        copy.receipt.current_id,
        load_checkpoint_in(&f.db, &s.root, &s.project_id, &first)
            .unwrap()
            .receipt
            .current_id
    );
    assert_eq!(
        resume_checkpoint(&mut f.db, &s.root, &s.project_id, &latest).unwrap(),
        s
    );
    assert_eq!(checkpoint_count(&f), 3);
    let invalid = CheckpointRecovery {
        source: first,
        copy_session_id: s.receipt.session_id.clone(),
    };
    assert!(recover_checkpoint(&mut f.db, &s.root, &s.project_id, &invalid, |_| Ok(())).is_err());
    assert_eq!(checkpoint_count(&f), 3);
}
#[test]
fn checkpoint_faults_roll_back_and_committed_retries_do_not_duplicate_rows() {
    for at in [
        "after_checkpoint_write",
        "before_checkpoint_commit",
        "after_checkpoint_commit",
    ] {
        let (mut f, s) = checkpoint_fixture();
        let before = s.clone();
        assert!(
            save_checkpoint(&mut f.db, &s, None, |point| if point == at {
                Err("injected".into())
            } else {
                Ok(())
            })
            .is_err()
        );
        f.reopen();
        f.reopen();
        assert_eq!(
            checkpoint_count(&f),
            i64::from(at == "after_checkpoint_commit")
        );
        assert_eq!(s, before);
        let r = save_checkpoint(&mut f.db, &s, None, |_| Ok(())).unwrap();
        assert_eq!(save_checkpoint(&mut f.db, &s, None, |_| Ok(())).unwrap(), r);
        assert_eq!(checkpoint_count(&f), 1);
    }
    for at in ["after_recovery_checkpoint", "before_recovery_commit"] {
        let (mut f, s) = checkpoint_fixture();
        let first = save_checkpoint(&mut f.db, &s, None, |_| Ok(())).unwrap();
        let choice =
            prepare_checkpoint_recovery(&mut f.db, &s.root, &s.project_id, &first).unwrap();
        assert!(
            recover_checkpoint(&mut f.db, &s.root, &s.project_id, &choice, |point| {
                if point == at {
                    Err("injected".into())
                } else {
                    Ok(())
                }
            })
            .is_err()
        );
        f.reopen();
        f.reopen();
        assert_eq!(checkpoint_count(&f), 1);
        let copy =
            recover_checkpoint(&mut f.db, &s.root, &s.project_id, &choice, |_| Ok(())).unwrap();
        assert_ne!(copy.session_id, first.session_id);
        assert_eq!(checkpoint_count(&f), 2);
    }
}
#[test]
fn future_corrupt_oversized_and_unknown_checkpoint_shapes_are_retained() {
    let (mut f, s) = checkpoint_fixture();
    let r = save_checkpoint(&mut f.db, &s, None, |_| Ok(())).unwrap();
    let (raw, hash) = checkpoint_row(&f.db, &s.root, &s.project_id, &r)
        .unwrap()
        .unwrap();
    let original: Value = serde_json::from_str(&raw).unwrap();
    for (pointer, replacement) in [
        ("/version", json!(2)),
        ("/project_revision", json!(JS_SAFE)),
        ("/document_hash", json!("a".repeat(64))),
        ("/session/root", json!("wrong-root")),
        ("/session/receipt/session_id", json!("bad/session")),
        ("/session/receipt/revision", json!(JS_SAFE + 1)),
        (
            "/session/undo",
            json!(vec![s.receipt.current_id.clone(); 21]),
        ),
    ] {
        let mut v = original.clone();
        *v.pointer_mut(pointer).unwrap() = replacement;
        let data = v.to_string();
        let hash = payload_hash(&data);
        f.db.execute(
            "UPDATE phase_j_session_checkpoints_test SET data=?1,hash=?2",
            params![data, hash],
        )
        .unwrap();
        let forged = CheckpointReceipt {
            hash: hash.clone(),
            ..r.clone()
        };
        assert!(
            resume_checkpoint(&mut f.db, &s.root, &s.project_id, &forged).is_err(),
            "{pointer}"
        );
        assert!(save_checkpoint(&mut f.db, &s, None, |_| Ok(())).is_err());
        assert_eq!(
            checkpoint_row(&f.db, &s.root, &s.project_id, &r)
                .unwrap()
                .unwrap(),
            (data, hash)
        );
    }
    for pointer in ["", "/session", "/session/receipt"] {
        let mut v = original.clone();
        v.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("future".into(), json!(true));
        let data = v.to_string();
        let hash = payload_hash(&data);
        f.db.execute(
            "UPDATE phase_j_session_checkpoints_test SET data=?1,hash=?2",
            params![data, hash],
        )
        .unwrap();
        let forged = CheckpointReceipt { hash, ..r.clone() };
        assert!(resume_checkpoint(&mut f.db, &s.root, &s.project_id, &forged).is_err());
    }
    f.db.execute(
        "UPDATE phase_j_session_checkpoints_test SET data=?1",
        ["x".repeat(CHECKPOINT_LIMIT + 1)],
    )
    .unwrap();
    assert!(resume_checkpoint(&mut f.db, &s.root, &s.project_id, &r).is_err());
    assert!(save_checkpoint(&mut f.db, &s, None, |_| Ok(())).is_err());
    f.db.execute(
        "UPDATE phase_j_session_checkpoints_test SET data=?1,hash=?2",
        params![raw, hash],
    )
    .unwrap();
    f.db.execute_batch("ALTER TABLE phase_j_session_checkpoints_test ADD COLUMN future TEXT")
        .unwrap();
    assert!(initialize_checkpoint_store(&f.db).is_err());
    assert!(resume_checkpoint(&mut f.db, &s.root, &s.project_id, &r).is_err());
    assert_eq!(checkpoint_count(&f), 1);
}
#[test]
fn stale_projects_foreign_roots_and_corrupt_inactive_history_block_resume_and_recovery() {
    let (mut f, mut s) = checkpoint_fixture();
    session_edit(&mut f, &mut s, Some(18)).unwrap();
    let r = save_checkpoint(&mut f.db, &s, None, |_| Ok(())).unwrap();
    let other = Fixture::new();
    assert!(resume_checkpoint(&mut f.db, other.root.to_str().unwrap(), &s.project_id, &r).is_err());
    assert!(resume_checkpoint(&mut f.db, &s.root, "foreign-project", &r).is_err());
    let choice = prepare_checkpoint_recovery(&mut f.db, &s.root, &s.project_id, &r).unwrap();
    let raw = f.raw_project();
    let mut v: Value = serde_json::from_str(&raw).unwrap();
    v["titles"][0]["text"] = json!("edited");
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![v.to_string(), s.project_id],
    )
    .unwrap();
    assert!(resume_checkpoint(&mut f.db, &s.root, &s.project_id, &r).is_err());
    assert!(save_checkpoint(&mut f.db, &s, Some(&r), |_| Ok(())).is_err());
    assert!(recover_checkpoint(&mut f.db, &s.root, &s.project_id, &choice, |_| Ok(())).is_err());
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![raw, s.project_id],
    )
    .unwrap();
    f.db.execute(
        "UPDATE phase_j_layer_manifests_test SET data='corrupt' WHERE id=?1",
        [&s.undo[0]],
    )
    .unwrap();
    assert!(resume_checkpoint(&mut f.db, &s.root, &s.project_id, &r).is_err());
    assert!(recover_checkpoint(&mut f.db, &s.root, &s.project_id, &choice, |_| Ok(())).is_err());
    assert_eq!(checkpoint_count(&f), 1);
}
#[test]
fn session_checkpoint_crash_child() {
    let Ok(root) = std::env::var("OPEN_PHOTO_CHECKPOINT_CRASH_ROOT") else {
        return;
    };
    let project = std::env::var("OPEN_PHOTO_CHECKPOINT_CRASH_PROJECT").unwrap();
    let at = std::env::var("OPEN_PHOTO_CHECKPOINT_CRASH_AT").unwrap();
    let receipt: CheckpointReceipt =
        serde_json::from_str(&std::env::var("OPEN_PHOTO_CHECKPOINT_CRASH_RECEIPT").unwrap())
            .unwrap();
    let mut db = Connection::open(PathBuf::from(&root).join("prototype.db")).unwrap();
    let mut s = resume_checkpoint(&mut db, &root, &project, &receipt).unwrap();
    let (project, _) = read(&db, &root, &project).unwrap();
    s.apply(
        &mut db,
        &s.receipt.clone(),
        SessionAction::Crossfade {
            from: &project.clips[0].id,
            to: &project.clips[1].id,
            frames: Some(18),
        },
        |_| Ok(()),
    )
    .unwrap();
    save_checkpoint(&mut db, &s, Some(&receipt), |point| {
        if point == at {
            std::process::exit(79);
        }
        Ok(())
    })
    .unwrap();
    panic!("checkpoint not reached");
}
#[test]
fn abrupt_checkpoint_exit_resumes_only_committed_state_and_keeps_unsaved_manifests() {
    for at in [
        "after_checkpoint_write",
        "before_checkpoint_commit",
        "after_checkpoint_commit",
    ] {
        let (mut f, s) = checkpoint_fixture();
        let base = save_checkpoint(&mut f.db, &s, None, |_| Ok(())).unwrap();
        let raw = f.raw_project();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "video_sequence::phase_j_sequence_binding::tests::session_checkpoint_crash_child",
                "--nocapture",
            ])
            .env("OPEN_PHOTO_CHECKPOINT_CRASH_ROOT", &f.root)
            .env("OPEN_PHOTO_CHECKPOINT_CRASH_PROJECT", &s.project_id)
            .env("OPEN_PHOTO_CHECKPOINT_CRASH_AT", at)
            .env(
                "OPEN_PHOTO_CHECKPOINT_CRASH_RECEIPT",
                serde_json::to_string(&base).unwrap(),
            )
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(79),
            "{at}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        f.reopen();
        f.reopen();
        assert_eq!(
            checkpoint_count(&f),
            if at == "after_checkpoint_commit" {
                2
            } else {
                1
            }
        );
        assert_eq!(manifest_count(&f), 2);
        let latest = latest_checkpoint(&f.db, &s.root, &s.project_id, &s.receipt.session_id)
            .unwrap()
            .unwrap();
        let mut resumed = resume_checkpoint(&mut f.db, &s.root, &s.project_id, &latest).unwrap();
        assert_eq!(
            resumed.receipt.revision,
            if at == "after_checkpoint_commit" {
                2
            } else {
                1
            }
        );
        if at == "after_checkpoint_commit" {
            resumed
                .apply(
                    &mut f.db,
                    &resumed.receipt.clone(),
                    SessionAction::Undo,
                    |_| Ok(()),
                )
                .unwrap();
            assert_eq!(resumed.receipt.current_id, s.receipt.current_id);
        }
        assert_eq!(f.raw_project(), raw);
    }
}
