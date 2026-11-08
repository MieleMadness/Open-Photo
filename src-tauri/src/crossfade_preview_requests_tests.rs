fn preview_submit(
    q: &PreviewRequests,
    f: &mut Fixture,
    s: &EditorSession,
    frame: u64,
) -> PreviewArc<PreviewRequest> {
    q.submit(&mut f.db, s, &s.receipt, SeekPoint::Frame(frame))
        .unwrap()
}
fn preview_start(
    q: &PreviewRequests,
    f: &mut Fixture,
    s: &EditorSession,
) -> PreviewArc<PreviewRequest> {
    q.start_next(&mut f.db, s, &s.receipt).unwrap().unwrap()
}
#[test]
fn preview_bursts_retain_one_worker_and_only_the_latest_pending_seek() {
    let (mut f, s) = seek_fixture(30, false);
    let q = PreviewRequests::new();
    let original = preview_submit(&q, &mut f, &s, 45);
    let active = preview_start(&q, &mut f, &s);
    assert!(PreviewArc::ptr_eq(&original, &active));
    let mut last = None;
    for n in 0..500 {
        let next = preview_submit(&q, &mut f, &s, n % 165);
        if let Some(previous) = last.replace(next) {
            assert!(previous.cancelled());
        }
        let state = q.state.lock().unwrap();
        assert!(state.active.is_some());
        assert!(state.pending.is_some());
    }
    let last = last.unwrap();
    assert!(active.cancelled());
    assert!(!last.cancelled());
    assert!(q.start_next(&mut f.db, &s, &s.receipt).unwrap().is_none());
    assert!(q
        .finish(
            &mut f.db,
            &s,
            &s.receipt,
            &active,
            Err("obsolete decode error".into())
        )
        .unwrap()
        .is_none());
    let started = preview_start(&q, &mut f, &s);
    assert!(PreviewArc::ptr_eq(&started, &last));
    assert!(!q.cancel(&active).unwrap());
    assert!(q
        .finish(&mut f.db, &s, &s.receipt, &active, Ok(vec![1]))
        .is_err());
    let delivered = q
        .finish(&mut f.db, &s, &s.receipt, &started, Ok(vec![1, 2, 3]))
        .unwrap()
        .unwrap();
    assert_eq!(delivered.position.project_frame, 499 % 165);
    assert_eq!(delivered.bytes, vec![1, 2, 3]);
    assert_eq!(delivered.owner, q.id);
    assert_eq!(delivered.generation, last.generation);
    assert!(q
        .still_current(&mut f.db, &s, &s.receipt, &delivered)
        .unwrap());
}
#[test]
fn preview_coalesces_equal_quantized_requests_and_invalid_input_cancels_old_work() {
    let (mut f, mut s) = seek_fixture(30, false);
    let q = PreviewRequests::new();
    let first = preview_submit(&q, &mut f, &s, 45);
    let same = q
        .submit(&mut f.db, &s, &s.receipt, SeekPoint::Seconds(1.51))
        .unwrap();
    assert!(PreviewArc::ptr_eq(&first, &same));
    let active = preview_start(&q, &mut f, &s);
    let again = preview_submit(&q, &mut f, &s, 45);
    assert!(PreviewArc::ptr_eq(&active, &again));
    assert!(!active.cancelled());
    assert!(q
        .submit(&mut f.db, &s, &s.receipt, SeekPoint::Seconds(f64::NAN))
        .is_err());
    assert!(active.cancelled());
    assert!(q
        .finish(&mut f.db, &s, &s.receipt, &active, Ok(vec![1]))
        .unwrap()
        .is_none());
    let next = preview_submit(&q, &mut f, &s, 46);
    preview_start(&q, &mut f, &s);
    let old = s.receipt.clone();
    session_edit(&mut f, &mut s, Some(18)).unwrap();
    assert!(q.submit(&mut f.db, &s, &old, SeekPoint::Frame(46)).is_err());
    assert!(next.cancelled());
    assert!(q
        .finish(&mut f.db, &s, &s.receipt, &next, Ok(vec![1]))
        .unwrap()
        .is_none());
}
#[test]
fn simulated_preview_worker_observes_cancellation_before_its_slot_is_reused() {
    let (mut f, s) = seek_fixture(30, false);
    let q = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 45);
    let active = preview_start(&q, &mut f, &s);
    let token = active.clone();
    let (release, wait) = std::sync::mpsc::channel();
    let (finished, completion) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        wait.recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(token.cancelled());
        finished
            .send(Err::<Vec<u8>, String>("cancelled worker".into()))
            .unwrap();
    });
    let latest = preview_submit(&q, &mut f, &s, 60);
    assert!(active.cancelled());
    assert!(q.start_next(&mut f.db, &s, &s.receipt).unwrap().is_none());
    release.send(()).unwrap();
    let result = completion
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    worker.join().unwrap();
    assert!(q
        .finish(&mut f.db, &s, &s.receipt, &active, result)
        .unwrap()
        .is_none());
    assert!(PreviewArc::ptr_eq(&preview_start(&q, &mut f, &s), &latest));
}
#[test]
fn preview_revalidates_native_state_before_worker_start_and_after_completion() {
    let (mut f, s) = seek_fixture(30, false);
    let q = PreviewRequests::new();
    let raw = f.raw_project();
    let pending = preview_submit(&q, &mut f, &s, 45);
    let mut changed: Value = serde_json::from_str(&raw).unwrap();
    changed["titles"][0]["text"] = json!("new title");
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![changed.to_string(), s.project_id],
    )
    .unwrap();
    assert!(q.start_next(&mut f.db, &s, &s.receipt).is_err());
    assert!(pending.cancelled());
    assert!(q.state.lock().unwrap().active.is_none());
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![raw, s.project_id],
    )
    .unwrap();
    let active = preview_submit(&q, &mut f, &s, 45);
    preview_start(&q, &mut f, &s);
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![changed.to_string(), s.project_id],
    )
    .unwrap();
    assert!(q
        .finish(&mut f.db, &s, &s.receipt, &active, Ok(vec![1]))
        .is_err());
    assert!(active.cancelled());
    assert!(q.state.lock().unwrap().active.is_none());
    assert_eq!(checkpoint_count_or_zero(&f), 0);
}
fn checkpoint_count_or_zero(f: &Fixture) -> i64 {
    if table_exists(&f.db, "phase_j_session_checkpoints_test").unwrap() {
        checkpoint_count(f)
    } else {
        0
    }
}
#[test]
fn preview_result_bounds_and_final_handoff_reject_late_or_failed_work() {
    let (mut f, mut s) = seek_fixture(30, false);
    let q = PreviewRequests::new();
    let count = manifest_count(&f);
    let raw = f.raw_project();
    for result in [
        Ok(vec![]),
        Ok(vec![0; PREVIEW_RESULT_LIMIT + 1]),
        Err("decoder failed".into()),
    ] {
        let active = preview_submit(&q, &mut f, &s, 45);
        preview_start(&q, &mut f, &s);
        assert!(q
            .finish(&mut f.db, &s, &s.receipt, &active, result)
            .is_err());
        assert!(q.state.lock().unwrap().active.is_none());
    }
    let active = preview_submit(&q, &mut f, &s, 45);
    preview_start(&q, &mut f, &s);
    let delivered = q
        .finish(&mut f.db, &s, &s.receipt, &active, Ok(vec![1]))
        .unwrap()
        .unwrap();
    assert!(q
        .still_current(&mut f.db, &s, &s.receipt, &delivered)
        .unwrap());
    assert_eq!(manifest_count(&f), count);
    assert_eq!(f.raw_project(), raw);
    let mut foreign = s.clone();
    foreign.root = "foreign-root".into();
    assert!(!q
        .still_current(&mut f.db, &foreign, &foreign.receipt, &delivered)
        .unwrap());
    let mut changed: Value = serde_json::from_str(&raw).unwrap();
    changed["titles"][0]["text"] = json!("changed before handoff");
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![changed.to_string(), s.project_id],
    )
    .unwrap();
    assert!(q
        .still_current(&mut f.db, &s, &s.receipt, &delivered)
        .is_err());
    f.db.execute(
        "UPDATE video_sequences SET data=?1 WHERE id=?2",
        params![raw, s.project_id],
    )
    .unwrap();
    preview_submit(&q, &mut f, &s, 60);
    assert!(!q
        .still_current(&mut f.db, &s, &s.receipt, &delivered)
        .unwrap());
    let active = preview_start(&q, &mut f, &s);
    let delivered = q
        .finish(&mut f.db, &s, &s.receipt, &active, Ok(vec![2]))
        .unwrap()
        .unwrap();
    session_edit(&mut f, &mut s, Some(18)).unwrap();
    assert!(!q
        .still_current(&mut f.db, &s, &s.receipt, &delivered)
        .unwrap());
    assert!(q
        .still_current(&mut f.db, &s, &delivered.position.session, &delivered)
        .is_err());
}
#[test]
fn preview_cancel_close_drop_and_foreign_tokens_preserve_worker_ownership() {
    let (mut f, s) = seek_fixture(30, false);
    let q = PreviewRequests::new();
    let other = PreviewRequests::new();
    preview_submit(&q, &mut f, &s, 45);
    let active = preview_start(&q, &mut f, &s);
    let foreign = preview_submit(&other, &mut f, &s, 45);
    preview_start(&other, &mut f, &s);
    assert!(!q.cancel(&foreign).unwrap());
    assert!(q
        .finish(&mut f.db, &s, &s.receipt, &foreign, Ok(vec![1]))
        .is_err());
    assert!(!active.cancelled());
    let queued = preview_submit(&q, &mut f, &s, 60);
    assert!(q.cancel(&active).unwrap());
    assert!(!queued.cancelled());
    q.close().unwrap();
    assert!(queued.cancelled());
    assert!(q
        .submit(&mut f.db, &s, &s.receipt, SeekPoint::Frame(61))
        .is_err());
    assert!(q
        .finish(&mut f.db, &s, &s.receipt, &active, Ok(vec![1]))
        .unwrap()
        .is_none());
    drop(other);
    assert!(foreign.cancelled());
    let poisoned = PreviewArc::new(PreviewRequests::new());
    let token = preview_submit(&poisoned, &mut f, &s, 45);
    let clone = poisoned.clone();
    assert!(std::thread::spawn(move || {
        let _guard = clone.state.lock().unwrap();
        panic!("simulated worker owner panic");
    })
    .join()
    .is_err());
    drop(poisoned);
    assert!(token.cancelled());
}
#[test]
fn preview_generation_exhaustion_fails_closed_and_new_sessions_do_not_coalesce() {
    let (mut f, s) = seek_fixture(30, false);
    let q = PreviewRequests::new();
    let original = preview_submit(&q, &mut f, &s, 45);
    preview_start(&q, &mut f, &s);
    let second =
        EditorSession::open(&mut f.db, &s.root, &s.project_id, &s.receipt.current_id).unwrap();
    let fresh = preview_submit(&q, &mut f, &second, 45);
    assert!(original.cancelled());
    assert!(!PreviewArc::ptr_eq(&original, &fresh));
    assert!(q
        .finish(&mut f.db, &second, &second.receipt, &original, Ok(vec![1]))
        .unwrap()
        .is_none());
    preview_start(&q, &mut f, &second);
    q.state.lock().unwrap().generation = JS_SAFE;
    assert!(q
        .submit(&mut f.db, &second, &second.receipt, SeekPoint::Frame(46))
        .is_err());
    assert!(fresh.cancelled());
    assert!(q.state.lock().unwrap().closed);
    assert!(q
        .finish(&mut f.db, &second, &second.receipt, &fresh, Ok(vec![1]))
        .unwrap()
        .is_none());
}
