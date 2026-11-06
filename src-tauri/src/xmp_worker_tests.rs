#[cfg(test)]
#[cfg(windows)]
mod worker_tests {
    use super::step2_apply_tests::Fixture;
    use super::*;
    use std::{
        future::Future,
        sync::mpsc,
        task::{Context, Waker},
        time::Duration,
    };
    const DEADLINE: Duration = Duration::from_secs(10);

    #[test]
    fn blocked_work_yields_the_caller_and_allows_another_task_to_finish() {
        let caller = std::thread::current().id();
        let (started, observed) = mpsc::channel();
        let (resume, gate) = mpsc::channel();
        let mut task = Box::pin(xmp_task(move || {
            started.send(std::thread::current().id()).unwrap();
            // A bounded gate, not a timing-based slow-file benchmark. If work
            // accidentally runs inline, the first poll returns Ready and fails.
            gate.recv_timeout(DEADLINE).map_err(|e| e.to_string())?;
            Ok(42)
        }));
        let mut context = Context::from_waker(Waker::noop());
        assert!(task.as_mut().poll(&mut context).is_pending());
        assert_ne!(observed.recv_timeout(DEADLINE).unwrap(), caller);
        assert_eq!(
            tauri::async_runtime::block_on(xmp_task(|| Ok(7))).unwrap(),
            7
        );
        resume.send(()).unwrap();
        assert_eq!(tauri::async_runtime::block_on(task).unwrap(), 42);
    }

    #[test]
    fn validation_errors_return_unchanged_without_a_success_value() {
        let result = tauri::async_runtime::block_on(xmp_task(|| {
            Err::<(), _>("The source changed; review again".to_string())
        }));
        assert_eq!(result.unwrap_err(), "The source changed; review again");
    }

    #[test]
    fn dropping_the_reply_does_not_cancel_an_approved_save_mid_transaction() {
        let mut fixture = Fixture::new();
        let (row, view) = fixture.proposal();
        let id = row.id;
        let candidate = row.candidate.clone();
        let original = row.original.clone().unwrap();
        let database = fixture.root.join("user-data.sqlite");
        let (started, observed) = mpsc::channel();
        let (resume, gate) = mpsc::channel();
        let (finished, completion) = mpsc::channel();
        let mut task = Box::pin(xmp_write_task(move || {
            started.send(()).unwrap();
            gate.recv_timeout(DEADLINE).map_err(|e| e.to_string())?;
            let mut db = Connection::open(database).map_err(|e| e.to_string())?;
            let result = apply_on(
                &mut db,
                "w",
                id,
                &view.fingerprint,
                &row.candidate_hash,
                |_, _| Ok(()),
            );
            // Main-thread cleanup must not race an open fixture database.
            drop(db);
            finished
                .send(result.map(|value| value.backup_name))
                .unwrap();
            Ok(())
        }));
        let mut context = Context::from_waker(Waker::noop());
        assert!(task.as_mut().poll(&mut context).is_pending());
        observed.recv_timeout(DEADLINE).unwrap();
        drop(task); // e.g. the requesting WebView goes away after confirmation.
        resume.send(()).unwrap();
        let backup = completion.recv_timeout(DEADLINE).unwrap().unwrap().unwrap();
        fixture.reopen();
        assert_eq!(load_review(&fixture.db, "w", id).unwrap().state, "applied");
        assert_eq!(fs::read_to_string(fixture.sidecar()).unwrap(), candidate);
        assert_eq!(
            fs::read_to_string(fixture.root.join(backup)).unwrap(),
            original
        );
        assert_eq!(fs::read(&fixture.source).unwrap(), b"original-media");
    }

    #[test]
    fn worker_panic_after_original_move_reports_failure_and_retains_recovery() {
        let mut fixture = Fixture::new();
        let (row, view) = fixture.proposal();
        let id = row.id;
        let original = row.original.clone().unwrap();
        let database = fixture.root.join("user-data.sqlite");
        let result = tauri::async_runtime::block_on(xmp_task(move || {
            let mut db = Connection::open(database).map_err(|e| e.to_string())?;
            apply_on(
                &mut db,
                "w",
                id,
                &view.fingerprint,
                &row.candidate_hash,
                |point, _| {
                    if point == "after_original_move" {
                        panic!("disposable worker failure");
                    }
                    Ok(())
                },
            )
        }));
        let error = result.unwrap_err();
        assert!(error.contains("Refresh saved changes"), "{error}");
        fixture.reopen();
        fixture.reopen();
        let saved = load_review(&fixture.db, "w", id).unwrap();
        assert_eq!(saved.state, "applying");
        assert!(!fixture.sidecar().exists());
        assert_eq!(
            fs::read_to_string(fixture.root.join(saved.backup_name.unwrap())).unwrap(),
            original
        );
        assert_eq!(fs::read(&fixture.source).unwrap(), b"original-media");
    }

    #[test]
    fn new_sidecar_and_later_decision_share_the_publication_lock() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.sidecar()).unwrap();
        let database = fixture.root.join("user-data.sqlite");
        let root = fixture.root.to_string_lossy().into_owned();
        let source = fixture.source.to_string_lossy().into_owned();
        let result = tauri::async_runtime::block_on(xmp_write_task(move || {
            let mut db = Connection::open(database).map_err(|e| e.to_string())?;
            batch_with_checkpoint(
                &mut db,
                "w".into(),
                root,
                vec![source],
                vec!["New".into()],
                |point, _| {
                    if matches!(
                        point,
                        "after_record_commit" | "before_publish" | "after_publish"
                    ) {
                        assert!(matches!(
                            crate::XMP_WRITE_LOCK.try_lock(),
                            Err(std::sync::TryLockError::WouldBlock)
                        ));
                    }
                    Ok(())
                },
            )
        }))
        .unwrap();
        assert_eq!(result.created, 1);
        let row = list_history(&fixture.db, "w", true).unwrap().remove(0);
        let database = fixture.root.join("user-data.sqlite");
        let decision = tauri::async_runtime::block_on(xmp_write_task(move || {
            assert!(matches!(
                crate::XMP_WRITE_LOCK.try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            ));
            let mut db = Connection::open(database).map_err(|e| e.to_string())?;
            review_decide_on(&mut db, "w", row.id, "cancelled", &row.candidate_hash)
        }));
        assert!(decision.is_err());
        assert_eq!(
            list_history(&fixture.db, "w", true).unwrap()[0].state,
            "published"
        );
        assert!(fs::read_to_string(fixture.sidecar())
            .unwrap()
            .contains("New"));
    }
}
