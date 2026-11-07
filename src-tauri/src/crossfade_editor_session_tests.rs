fn session_fixture() -> (Fixture, EditorSession) {
    let (mut f, data) = manifest_fixture();
    let id = save_layer_manifest(
        &mut f.db,
        f.root.to_str().unwrap(),
        &f.project.id,
        &data,
        |_| Ok(()),
    )
    .unwrap();
    let session =
        EditorSession::open(&mut f.db, f.root.to_str().unwrap(), &f.project.id, &id).unwrap();
    (f, session)
}
fn session_edit(
    f: &mut Fixture,
    s: &mut EditorSession,
    frames: Option<u64>,
) -> Result<SessionReceipt, String> {
    s.apply(
        &mut f.db,
        &s.receipt.clone(),
        SessionAction::Crossfade {
            from: &f.project.clips[0].id,
            to: &f.project.clips[1].id,
            frames,
        },
        |_| Ok(()),
    )
}
#[test]
fn session_edits_selection_and_undo_redo_preserve_native_projects() {
    let (mut f, mut s) = session_fixture();
    let raw = f.raw_project();
    let first = s.receipt.clone();
    let second = session_edit(&mut f, &mut s, Some(18)).unwrap();
    assert_eq!(second.revision, 2);
    assert_ne!(first.current_id, second.current_id);
    let removed = session_edit(&mut f, &mut s, None).unwrap();
    assert_eq!(removed.revision, 3);
    assert_eq!(
        load_layer_manifest(&mut f.db, &s.root, &s.project_id, &removed.current_id)
            .unwrap()
            .boundaries
            .len(),
        0
    );
    let undo = s
        .apply(&mut f.db, &s.receipt.clone(), SessionAction::Undo, |_| {
            Ok(())
        })
        .unwrap();
    assert_eq!(undo.current_id, second.current_id);
    assert_eq!(undo.revision, 4);
    let redo = s
        .apply(&mut f.db, &s.receipt.clone(), SessionAction::Redo, |_| {
            Ok(())
        })
        .unwrap();
    assert_eq!(redo.current_id, removed.current_id);
    let selected = s
        .apply(
            &mut f.db,
            &s.receipt.clone(),
            SessionAction::Select(&first.current_id),
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(selected.current_id, first.current_id);
    assert_eq!(s.undo.len(), 3);
    assert!(s.redo.is_empty());
    assert_eq!(manifest_count(&f), 3);
    assert_eq!(f.raw_project(), raw);
    assert_eq!(f.history(), 0);
}
#[test]
fn noops_preserve_revision_and_redo_while_new_edits_branch() {
    let (mut f, mut s) = session_fixture();
    let initial = s.receipt.current_id.clone();
    session_edit(&mut f, &mut s, Some(18)).unwrap();
    s.apply(&mut f.db, &s.receipt.clone(), SessionAction::Undo, |_| {
        Ok(())
    })
    .unwrap();
    let before = (
        s.receipt.clone(),
        s.undo.clone(),
        s.redo.clone(),
        manifest_count(&f),
    );
    assert_eq!(session_edit(&mut f, &mut s, Some(15)).unwrap(), before.0);
    s.apply(
        &mut f.db,
        &s.receipt.clone(),
        SessionAction::Select(&initial),
        |_| panic!("no-op checkpoint"),
    )
    .unwrap();
    assert_eq!(
        (
            s.receipt.clone(),
            s.undo.clone(),
            s.redo.clone(),
            manifest_count(&f)
        ),
        before
    );
    session_edit(&mut f, &mut s, Some(21)).unwrap();
    assert!(s.redo.is_empty());
    let receipt = s.receipt.clone();
    assert!(s
        .apply(&mut f.db, &receipt, SessionAction::Redo, |_| Ok(()))
        .is_err());
    assert_eq!(s.receipt, receipt);
}
#[test]
fn stale_cross_session_receipts_and_revision_overflow_do_not_mutate_state() {
    let (mut f, mut s) = session_fixture();
    let initial = s.receipt.clone();
    let other =
        EditorSession::open(&mut f.db, &s.root, &s.project_id, &initial.current_id).unwrap();
    assert!(s
        .apply(&mut f.db, &other.receipt, SessionAction::Undo, |_| Ok(()))
        .is_err());
    session_edit(&mut f, &mut s, Some(18)).unwrap();
    let before = (s.receipt.clone(), s.undo.clone(), manifest_count(&f));
    assert!(s
        .apply(&mut f.db, &initial, SessionAction::Undo, |_| Ok(()))
        .is_err());
    assert_eq!(
        (s.receipt.clone(), s.undo.clone(), manifest_count(&f)),
        before
    );
    s.receipt.revision = JS_SAFE;
    let receipt = s.receipt.clone();
    let count = manifest_count(&f);
    assert!(session_edit(&mut f, &mut s, Some(21)).is_err());
    assert_eq!(s.receipt, receipt);
    assert_eq!(manifest_count(&f), count);
    assert_eq!(session_edit(&mut f, &mut s, Some(18)).unwrap(), receipt);
}
#[test]
fn invalid_transition_edits_and_foreign_selection_never_create_history() {
    let (mut f, mut s) = session_fixture();
    let before = s.receipt.clone();
    for (from, to, frames) in [
        (
            f.project.clips[0].id.as_str(),
            f.project.clips[1].id.as_str(),
            Some(0),
        ),
        (f.project.clips[0].id.as_str(), "photo-clip", None),
        (f.project.clips[1].id.as_str(), "photo-clip", Some(5)), // saved dip at this cut
        (
            f.project.clips[0].id.as_str(),
            f.project.clips[1].id.as_str(),
            Some(53),
        ),
    ] {
        assert!(s
            .apply(
                &mut f.db,
                &before,
                SessionAction::Crossfade { from, to, frames },
                |_| Ok(())
            )
            .is_err());
    }
    assert!(s
        .apply(
            &mut f.db,
            &before,
            SessionAction::Select(&"f".repeat(64)),
            |_| Ok(())
        )
        .is_err());
    assert!(s
        .apply(&mut f.db, &before, SessionAction::Undo, |_| Ok(()))
        .is_err());
    assert_eq!(s.receipt, before);
    assert!(s.undo.is_empty());
    assert!(s.redo.is_empty());
    assert_eq!(manifest_count(&f), 1);
}
#[test]
fn stale_native_projects_and_corrupt_undo_targets_preserve_session_history() {
    let (mut f, mut s) = session_fixture();
    let first = s.receipt.current_id.clone();
    session_edit(&mut f, &mut s, Some(18)).unwrap();
    let before = (s.receipt.clone(), s.undo.clone(), s.redo.clone());
    f.db.execute(
        "UPDATE phase_j_layer_manifests_test SET data='corrupt' WHERE id=?1",
        [&first],
    )
    .unwrap();
    assert!(s
        .apply(&mut f.db, &before.0, SessionAction::Undo, |_| Ok(()))
        .is_err());
    assert_eq!((s.receipt.clone(), s.undo.clone(), s.redo.clone()), before);
    let raw = f.raw_project();
    let mut changed: Value = serde_json::from_str(&raw).unwrap();
    changed["titles"][0]["text"] = json!("same-revision title edit");
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![changed.to_string(), f.project.id],
    )
    .unwrap();
    for action in [
        SessionAction::Select(&s.receipt.current_id.clone()),
        SessionAction::Undo,
        SessionAction::Redo,
    ] {
        assert!(s.apply(&mut f.db, &before.0, action, |_| Ok(())).is_err());
    }
    assert!(session_edit(&mut f, &mut s, Some(21)).is_err());
    assert_eq!((s.receipt.clone(), s.undo.clone(), s.redo.clone()), before);
    assert_eq!(manifest_count(&f), 2);
}
#[test]
fn session_faults_preserve_state_until_commit_and_lost_reply_fences_retry() {
    for at in [
        "after_session_manifest",
        "before_session_commit",
        "after_session_commit",
    ] {
        let (mut f, mut s) = session_fixture();
        let before = s.receipt.clone();
        let from = f.project.clips[0].id.clone();
        let to = f.project.clips[1].id.clone();
        let action = SessionAction::Crossfade {
            from: &from,
            to: &to,
            frames: Some(18),
        };
        assert!(s
            .apply(&mut f.db, &before, action, |point| if point == at {
                Err("injected".into())
            } else {
                Ok(())
            })
            .is_err());
        f.reopen();
        f.reopen();
        if at == "after_session_commit" {
            assert_eq!(s.receipt.revision, 2);
            assert_eq!(manifest_count(&f), 2);
            assert_eq!(s.undo, vec![before.current_id.clone()]);
            assert!(s.apply(&mut f.db, &before, action, |_| Ok(())).is_err());
            assert_eq!(session_edit(&mut f, &mut s, Some(18)).unwrap(), s.receipt);
        } else {
            assert_eq!(s.receipt, before);
            assert_eq!(manifest_count(&f), 1);
            assert!(s.undo.is_empty());
            s.apply(&mut f.db, &before, action, |_| Ok(())).unwrap();
        }
        let reopened =
            EditorSession::open(&mut f.db, &s.root, &s.project_id, &s.receipt.current_id).unwrap();
        assert_eq!(reopened.receipt.current_id, s.receipt.current_id);
        assert_ne!(reopened.receipt.session_id, s.receipt.session_id);
        assert!(reopened.undo.is_empty());
    }
}
#[test]
fn undo_window_is_bounded_without_pruning_immutable_snapshots() {
    let (mut f, mut s) = session_fixture();
    let initial = s.receipt.current_id.clone();
    for frames in 16..=40 {
        session_edit(&mut f, &mut s, Some(frames)).unwrap();
    }
    assert_eq!(s.undo.len(), SESSION_UNDO_LIMIT);
    assert_eq!(manifest_count(&f), 26);
    load_layer_manifest(&mut f.db, &s.root, &s.project_id, &initial).unwrap();
    for _ in 0..SESSION_UNDO_LIMIT {
        s.apply(&mut f.db, &s.receipt.clone(), SessionAction::Undo, |_| {
            Ok(())
        })
        .unwrap();
    }
    assert!(s
        .apply(&mut f.db, &s.receipt.clone(), SessionAction::Undo, |_| Ok(
            ()
        ))
        .is_err());
    assert_eq!(s.redo.len(), SESSION_UNDO_LIMIT);
    for _ in 0..SESSION_UNDO_LIMIT {
        s.apply(&mut f.db, &s.receipt.clone(), SessionAction::Redo, |_| {
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(manifest_count(&f), 26);
    assert_eq!(s.undo.len(), SESSION_UNDO_LIMIT);
}
