// Regression coverage for the three step 1 recovery findings.
#[cfg(all(test, windows))]
mod step1_review_audit {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        root: PathBuf,
        source: PathBuf,
        db: Connection,
    }
    impl Fixture {
        fn new() -> Self {
            let root = loop {
                let candidate = std::env::temp_dir().join(format!(
                    "openphoto-xmp-step1-review-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&candidate) {
                    Ok(()) => break candidate,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Could not create disposable fixture: {error}"),
                }
            };
            let source = root.join("sample.jpg");
            fs::write(&source, b"disposable-original").unwrap();
            let db = Connection::open_in_memory().unwrap();
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
        fn run(&mut self) -> Batch {
            batch(
                &mut self.db,
                "w".into(),
                self.root.to_string_lossy().into(),
                vec![self.source.to_string_lossy().into()],
                vec!["New".into()],
            )
            .unwrap()
        }
        fn sidecar(&self) -> PathBuf {
            self.root.join("sample.jpg.xmp")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
    #[test]
    fn one_corrupt_record_does_not_hide_valid_pending_reviews() {
        let f = Fixture::new();
        let root = f.root.to_str().unwrap();
        let first = record(
            &f.db,
            "w",
            root,
            "first.jpg",
            Some("old"),
            "candidate-one",
            Some("id-one"),
        )
        .unwrap();
        record(
            &f.db,
            "w",
            root,
            "second.jpg",
            Some("old"),
            "candidate-two",
            Some("id-two"),
        )
        .unwrap();
        f.db.execute(
            "UPDATE xmp_operations SET candidate='damaged' WHERE id=?1",
            [first],
        )
        .unwrap();
        let rows = list(&f.db, "w").unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows
            .iter()
            .find(|r| r.id == first)
            .unwrap()
            .integrity_error
            .is_some());
        assert!(rows
            .iter()
            .find(|r| r.id != first)
            .unwrap()
            .integrity_error
            .is_none());
        let healthy: i64 =
            f.db.query_row(
                "SELECT count(*) FROM xmp_operations WHERE candidate='candidate-two'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(healthy, 1);
    }
    #[test]
    fn cancel_succeeds_without_opening_moved_source_media() {
        let mut f = Fixture::new();
        let old = crate::render_openphoto_xmp(&["Old".into()]);
        fs::write(f.sidecar(), &old).unwrap();
        assert_eq!(f.run().review, 1);
        let row = list(&f.db, "w").unwrap().remove(0);
        let id = row.id;
        let fingerprint = row.candidate_hash;
        let moved = f.root.join("moved.jpg");
        fs::rename(&f.source, &moved).unwrap();
        review_decide_on(&mut f.db, "w", id, "cancelled", &fingerprint).unwrap();
        assert!(list(&f.db, "w").unwrap().is_empty());
        assert_eq!(
            list_history(&f.db, "w", true).unwrap()[0].state,
            "cancelled"
        );
        assert_eq!(fs::read(&moved).unwrap(), b"disposable-original");
        assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), old);
    }
    #[test]
    fn completed_history_exposes_missing_acknowledged_sidecar() {
        let mut f = Fixture::new();
        let result = f.run();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.created, 1);
        fs::remove_file(f.sidecar()).unwrap();
        assert!(list(&f.db, "w").unwrap().is_empty());
        let saved: String =
            f.db.query_row(
                "SELECT candidate FROM xmp_operations WHERE state='published'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(saved.contains("New"));
        let history = list_history(&f.db, "w", true).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(
            review_snapshot_on(&mut f.db, "w", history[0].id)
                .unwrap()
                .observation,
            "missing"
        );
    }
}

#[cfg(all(test, windows))]
mod step2_protocol_experiment {
    use super::*;
    #[test]
    fn owned_delete_handle_can_move_old_sidecar_without_unlocking_it() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
        let root =
            std::env::temp_dir().join(format!("openphoto-step2-owned-move-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("sample.jpg");
        let sidecar = root.join("sample.jpg.xmp");
        let backup = root.join("original.review.xmp");
        fs::write(&source, b"original-media").unwrap();
        fs::write(&sidecar, b"original-xmp").unwrap();
        let mut guard = Guard::open(root.to_str().unwrap(), source.to_str().unwrap()).unwrap();
        drop(guard.sidecar.take());
        let protected = fs::OpenOptions::new()
            .read(true)
            .access_mode(0x80010000)
            .share_mode(FILE_SHARE_READ)
            .open(&sidecar)
            .unwrap();
        assert_eq!(
            fs::OpenOptions::new()
                .write(true)
                .open(&sidecar)
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
        rename_owned_no_replace(&protected, &backup, guard._directories.last().unwrap()).unwrap();
        assert!(!sidecar.exists());
        assert_eq!(fs::read(&backup).unwrap(), b"original-xmp");
        assert_eq!(
            fs::OpenOptions::new()
                .write(true)
                .open(&backup)
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
        rename_owned_no_replace(&protected, &sidecar, guard._directories.last().unwrap()).unwrap();
        drop(protected);
        drop(guard);
        assert_eq!(fs::read(&sidecar).unwrap(), b"original-xmp");
        assert_eq!(fs::read(&source).unwrap(), b"original-media");
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, windows))]
mod step2_apply_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    pub(super) struct Fixture {
        pub(super) root: PathBuf,
        pub(super) source: PathBuf,
        pub(super) db: Connection,
    }
    impl Fixture {
        pub(super) fn new() -> Self {
            let root = loop {
                let candidate = std::env::temp_dir().join(format!(
                    "openphoto-step2-apply-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&candidate) {
                    Ok(()) => break candidate,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Could not create disposable fixture: {error}"),
                }
            };
            let source = root.join("photo.jpg");
            fs::write(&source, b"original-media").unwrap();
            let original = crate::render_openphoto_xmp(&["Old".into()]).replace(
                "</rdf:Description>",
                "<third:keep xmlns:third=\"urn:third\">untouched</third:keep></rdf:Description>",
            );
            fs::write(root.join("photo.jpg.xmp"), original).unwrap();
            let db = Connection::open(root.join("user-data.sqlite")).unwrap();
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
        pub(super) fn sidecar(&self) -> PathBuf {
            self.root.join("photo.jpg.xmp")
        }
        pub(super) fn proposal(&mut self) -> (Review, Snapshot) {
            let result = batch(
                &mut self.db,
                "w".into(),
                self.root.to_string_lossy().into(),
                vec![self.source.to_string_lossy().into()],
                vec!["New".into()],
            )
            .unwrap();
            assert!(result.errors.is_empty(), "{:?}", result.errors);
            assert_eq!(result.review, 1);
            let row = list(&self.db, "w").unwrap().remove(0);
            let snapshot = review_snapshot_on(&mut self.db, "w", row.id).unwrap();
            assert!(snapshot.can_apply, "{:?}", snapshot.reason);
            (row, snapshot)
        }
        pub(super) fn apply(&mut self, row: &Review, view: &Snapshot) -> Result<Applied, String> {
            apply_on(
                &mut self.db,
                "w",
                row.id,
                &view.fingerprint,
                &row.candidate_hash,
                |_, _| Ok(()),
            )
        }
        pub(super) fn reopen(&mut self) {
            self.db = Connection::open(self.root.join("user-data.sqlite")).unwrap();
            schema(&self.db).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            assert_eq!(fs::read(&self.source).unwrap(), b"original-media");
            self.db = Connection::open_in_memory().unwrap();
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
    #[test]
    fn applies_reviewed_merge_and_retains_exact_original() {
        let mut f = Fixture::new();
        let original = fs::read(f.sidecar()).unwrap();
        let (row, view) = f.proposal();
        let result = f.apply(&row, &view).unwrap();
        assert_eq!(
            fs::read(f.root.join(result.backup_name.unwrap())).unwrap(),
            original
        );
        let current = fs::read_to_string(f.sidecar()).unwrap();
        assert!(current.contains("third:keep"));
        assert_eq!(
            crate::openphoto_xmp_tags(&current).unwrap(),
            vec!["Old", "New"]
        );
        assert!(list(&f.db, "w").unwrap().is_empty());
        assert_eq!(list_history(&f.db, "w", true).unwrap()[0].state, "applied");
        assert!(f.apply(&row, &view).is_err());
        assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), current);
    }
    #[test]
    fn legacy_artifacts_block_apply_and_pending_identity_blocks_path_aliases() {
        let mut f = Fixture::new();
        let (row, view) = f.proposal();
        let old = fs::read(f.sidecar()).unwrap();
        let backup = f.root.join("photo.jpg.xmp.openphoto.bak");
        fs::write(&backup, b"legacy-original").unwrap();
        assert!(
            !review_snapshot_on(&mut f.db, "w", row.id)
                .unwrap()
                .can_apply
        );
        assert!(f.apply(&row, &view).is_err());
        assert_eq!(fs::read(&backup).unwrap(), b"legacy-original");
        assert_eq!(fs::read(f.sidecar()).unwrap(), old);
        let result = batch(
            &mut f.db,
            "w".into(),
            f.root.to_string_lossy().into(),
            vec![f.source.to_string_lossy().to_uppercase()],
            vec!["More".into()],
        )
        .unwrap();
        assert_eq!(result.errors.len(), 1);
        assert_eq!(list(&f.db, "w").unwrap().len(), 1);
    }
    #[test]
    fn live_parent_junction_conversion_is_refused_at_apply_boundary() {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::{
            Storage::FileSystem::{
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
                FILE_SHARE_READ, FILE_SHARE_WRITE,
            },
            System::IO::DeviceIoControl,
        };
        let mut f = Fixture::new();
        let outside = Fixture::new();
        let outside_before = fs::read(outside.sidecar()).unwrap();
        let template = f.root.join("junction-template");
        let output = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&template)
            .arg(&outside.root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let reparse = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&template)
            .unwrap();
        let mut buffer = vec![0u8; 16 * 1024];
        let mut returned = 0;
        // SAFETY: synchronous live handle, initialized writable output storage.
        assert_ne!(
            unsafe {
                DeviceIoControl(
                    reparse.as_raw_handle(),
                    0x900a8,
                    std::ptr::null(),
                    0,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            },
            0
        );
        buffer.truncate(returned as usize);
        drop(reparse);
        fs::remove_dir(&template).unwrap();
        let (row, view) = f.proposal();
        let parent = f.root.clone();
        apply_on(&mut f.db,"w",row.id,&view.fingerprint,&row.candidate_hash,|point,_|{
            if point=="before_original_move" {
                let writer=fs::OpenOptions::new().write(true).share_mode(FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE).custom_flags(FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT).open(&parent).unwrap();
                // SAFETY: copied valid junction buffer from a disposable fixture;
                // the destination is the retained nonempty media parent.
                let result=unsafe{DeviceIoControl(writer.as_raw_handle(),0x900a4,buffer.as_ptr().cast(),buffer.len() as u32,std::ptr::null_mut(),0,&mut returned,std::ptr::null_mut())};
                if result!=0 {
                    let mut deletion=[0u8;8];deletion[..4].copy_from_slice(&buffer[..4]);
                    // SAFETY: remove only the tag just installed on this fixture.
                    assert_ne!(unsafe{DeviceIoControl(writer.as_raw_handle(),0x900ac,deletion.as_ptr().cast(),8,std::ptr::null_mut(),0,&mut returned,std::ptr::null_mut())},0);
                    panic!("Live directory conversion succeeded; Apply must remain blocked until this case is protected");
                }
                let error=std::io::Error::last_os_error();assert!(matches!(error.raw_os_error(),Some(145|5)),"Unexpected live-reparse refusal: {error}");eprintln!("Live parent junction conversion refused under Apply guard: {error}");
            }Ok(())
        }).unwrap();
        assert_eq!(fs::read(outside.sidecar()).unwrap(), outside_before);
        assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), row.candidate);
    }
    #[test]
    fn protected_apply_external_process_child() {
        let Some(root) = std::env::var_os("OPENPHOTO_APPLY_EDITOR_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let sidecar = root.join("photo.jpg.xmp");
        let stage = root.join("external-stage.tmp");
        fs::write(&stage, b"external-editor-replacement").unwrap();
        assert_eq!(
            fs::OpenOptions::new()
                .write(true)
                .open(&sidecar)
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
        assert_eq!(
            crate::atomic_replace(&stage, &sidecar)
                .unwrap_err()
                .raw_os_error(),
            Some(32)
        );
    }
    #[test]
    fn external_process_cannot_edit_or_replace_the_reviewed_sidecar_before_move() {
        let mut f = Fixture::new();
        let (row, view) = f.proposal();
        let original = fs::read(f.sidecar()).unwrap();
        let root = f.root.clone();
        let result=apply_on(&mut f.db,"w",row.id,&view.fingerprint,&row.candidate_hash,|point,_|{
            if point=="before_original_move"{
                let mut child=std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","xmp_transactions::step2_apply_tests::protected_apply_external_process_child","--nocapture"]).env("OPENPHOTO_APPLY_EDITOR_ROOT",&root).spawn().unwrap();let deadline=std::time::Instant::now()+std::time::Duration::from_secs(20);loop{if let Some(status)=child.try_wait().unwrap(){assert!(status.success());break}
if std::time::Instant::now()>deadline{child.kill().unwrap();child.wait().unwrap();panic!("External editor helper timed out")};std::thread::sleep(std::time::Duration::from_millis(10));}
            }Ok(())
        }).unwrap();
        assert_eq!(
            fs::read(f.root.join(result.backup_name.unwrap())).unwrap(),
            original
        );
        assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), row.candidate);
    }
    #[test]
    fn rejects_stale_sidecar_proposal_and_source_identities() {
        let mut f = Fixture::new();
        let (row, view) = f.proposal();
        let external = crate::render_openphoto_xmp(&["External".into()]);
        fs::write(f.sidecar(), &external).unwrap();
        assert!(f.apply(&row, &view).is_err());
        assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), external);
        assert!(
            !review_snapshot_on(&mut f.db, "w", row.id)
                .unwrap()
                .can_apply
        );
        let mut f = Fixture::new();
        let (row, view) = f.proposal();
        fs::rename(&f.source, f.root.join("retained-photo.jpg")).unwrap();
        fs::write(&f.source, b"original-media").unwrap();
        assert!(f.apply(&row, &view).is_err());
        assert!(
            !review_snapshot_on(&mut f.db, "w", row.id)
                .unwrap()
                .can_apply
        );
        let mut f = Fixture::new();
        let (row, view) = f.proposal();
        f.db.execute(
            "UPDATE xmp_operations SET candidate=?1,candidate_hash=?2 WHERE id=?3",
            params![
                crate::render_openphoto_xmp(&["Other".into()]),
                hash(crate::render_openphoto_xmp(&["Other".into()]).as_bytes()),
                row.id
            ],
        )
        .unwrap();
        assert!(f.apply(&row, &view).is_err());
    }
    #[test]
    fn competing_file_after_protected_move_wins_without_overwrite() {
        let mut f = Fixture::new();
        let original = fs::read(f.sidecar()).unwrap();
        let (row, view) = f.proposal();
        let result = apply_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, path| {
                if point == "after_original_move" {
                    fs::write(path, b"external-winner").unwrap();
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read(f.sidecar()).unwrap(), b"external-winner");
        let interrupted = load_review(&f.db, "w", row.id).unwrap();
        assert_eq!(interrupted.state, "applying");
        assert_eq!(
            fs::read(f.root.join(interrupted.backup_name.unwrap())).unwrap(),
            original
        );
        assert!(
            !review_snapshot_on(&mut f.db, "w", row.id)
                .unwrap()
                .can_apply
        );
    }
    #[test]
    fn interrupted_publication_can_be_acknowledged_only_after_fresh_current_review() {
        let mut f = Fixture::new();
        let (row, view) = f.proposal();
        let result = apply_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, _| {
                if point == "after_apply_publish" {
                    Err("acknowledgement interrupted".into())
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.is_err());
        let current = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(!current.can_apply);
        assert_eq!(current.current.as_deref(), Some(row.candidate.as_str()));
        let backup = load_review(&f.db, "w", row.id)
            .unwrap()
            .backup_name
            .unwrap();
        let before = fs::read(f.root.join(&backup)).unwrap();
        review_decide_on(
            &mut f.db,
            "w",
            row.id,
            "kept_external",
            &current.fingerprint,
        )
        .unwrap();
        assert!(list(&f.db, "w").unwrap().is_empty());
        assert_eq!(fs::read(f.root.join(backup)).unwrap(), before);
        assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), row.candidate);
    }
    #[test]
    fn handled_failure_restores_original_only_when_destination_is_free() {
        let mut f = Fixture::new();
        let original = fs::read(f.sidecar()).unwrap();
        let (row, view) = f.proposal();
        let error = apply_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, _| {
                if point == "after_original_move" {
                    Err("injected failure".into())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert!(error.contains("original was restored"));
        assert_eq!(fs::read(f.sidecar()).unwrap(), original);
        assert_eq!(
            load_review(&f.db, "w", row.id).unwrap().state,
            "review_required"
        );
        let fresh = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(fresh.can_apply);
        f.apply(&row, &fresh).unwrap();
    }
    #[test]
    fn preexisting_writer_and_removed_registration_refuse_apply() {
        let mut f = Fixture::new();
        let (row, view) = f.proposal();
        let writer = fs::OpenOptions::new()
            .write(true)
            .open(f.sidecar())
            .unwrap();
        assert!(f.apply(&row, &view).is_err());
        drop(writer);
        f.db.execute(
            "UPDATE user_data SET value='[{\"id\":\"w\",\"roots\":[]}]'",
            [],
        )
        .unwrap();
        assert!(f.apply(&row, &view).is_err());
        review_decide_on(&mut f.db, "w", row.id, "cancelled", &row.candidate_hash).unwrap();
        assert!(list(&f.db, "w").unwrap().is_empty());
    }
    #[test]
    fn missing_destination_uses_create_new_and_preserves_competing_file() {
        let mut f = Fixture::new();
        fs::remove_file(f.sidecar()).unwrap();
        let result = batch_with_checkpoint(
            &mut f.db,
            "w".into(),
            f.root.to_string_lossy().into(),
            vec![f.source.to_string_lossy().into()],
            vec!["New".into()],
            |point, _| {
                if point == "after_record_commit" {
                    Err("pending new sidecar".into())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
        assert_eq!(result.errors.len(), 1);
        let row = list(&f.db, "w").unwrap().remove(0);
        let view = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(view.can_apply);
        let result = apply_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, path| {
                if point == "before_publish" {
                    fs::write(path, b"external-new-sidecar").unwrap();
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read(f.sidecar()).unwrap(), b"external-new-sidecar");
    }
    #[test]
    fn journal_faults_never_hide_ambiguous_publication() {
        for boundary in [
            "before_apply_intent_commit",
            "after_apply_intent_commit",
            "before_original_move",
            "after_original_move",
            "after_stage_create",
            "after_stage_write",
            "after_stage_flush",
            "before_stage_reopen",
            "before_publish",
            "after_apply_publish",
            "before_apply_acknowledgement",
        ] {
            let mut f = Fixture::new();
            let original = fs::read(f.sidecar()).unwrap();
            let (row, view) = f.proposal();
            let result = apply_on(
                &mut f.db,
                "w",
                row.id,
                &view.fingerprint,
                &row.candidate_hash,
                |point, _| {
                    if point == boundary {
                        Err(format!("injected {point}"))
                    } else {
                        Ok(())
                    }
                },
            );
            assert!(result.is_err(), "{boundary}");
            f.reopen();
            let stored = load_review(&f.db, "w", row.id).unwrap();
            assert_eq!(stored.original.as_deref().unwrap().as_bytes(), original);
            assert_eq!(stored.candidate, row.candidate);
            assert_ne!(stored.state, "applied");
            assert_eq!(list(&f.db, "w").unwrap().len(), 1);
            if matches!(
                boundary,
                "after_apply_publish" | "before_apply_acknowledgement"
            ) {
                assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), row.candidate);
                assert_eq!(
                    fs::read(f.root.join(stored.backup_name.unwrap())).unwrap(),
                    original
                );
            } else {
                assert_eq!(fs::read(f.sidecar()).unwrap(), original);
            }
        }
    }
    #[test]
    fn abrupt_apply_exit_child() {
        let Some(root) = std::env::var_os("OPENPHOTO_APPLY_CRASH_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let mut db = Connection::open(root.join("user-data.sqlite")).unwrap();
        schema(&db).unwrap();
        let row = list(&db, "w").unwrap().remove(0);
        let view = review_snapshot_on(&mut db, "w", row.id).unwrap();
        let boundary = std::env::var("OPENPHOTO_APPLY_CRASH_BOUNDARY").unwrap();
        let _ = apply_on(
            &mut db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, _| {
                if point == boundary {
                    std::process::exit(77)
                }
                Ok(())
            },
        );
        panic!("Apply crash boundary not reached");
    }
    #[test]
    fn cold_restart_preserves_original_after_abrupt_apply_exit() {
        for boundary in [
            "before_apply_intent_commit",
            "after_apply_intent_commit",
            "after_original_move",
            "after_stage_flush",
            "after_apply_publish",
            "before_apply_acknowledgement",
        ] {
            let mut f = Fixture::new();
            let original = fs::read(f.sidecar()).unwrap();
            let (row, _) = f.proposal();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "xmp_transactions::step2_apply_tests::abrupt_apply_exit_child",
                    "--nocapture",
                ])
                .env("OPENPHOTO_APPLY_CRASH_ROOT", &f.root)
                .env("OPENPHOTO_APPLY_CRASH_BOUNDARY", boundary)
                .spawn()
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if std::time::Instant::now() > deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("Apply helper timed out")
                };
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            assert_eq!(status.code(), Some(77));
            f.reopen();
            let saved = load_review(&f.db, "w", row.id).unwrap();
            assert_eq!(saved.original.as_deref().unwrap().as_bytes(), original);
            assert_eq!(list(&f.db, "w").unwrap().len(), 1);
            match boundary {
                "after_original_move" | "after_stage_flush" => {
                    assert!(!f.sidecar().exists());
                    assert_eq!(
                        fs::read(f.root.join(saved.backup_name.as_ref().unwrap())).unwrap(),
                        original
                    )
                }
                "after_apply_publish" | "before_apply_acknowledgement" => {
                    assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), row.candidate);
                    assert_eq!(
                        fs::read(f.root.join(saved.backup_name.as_ref().unwrap())).unwrap(),
                        original
                    )
                }
                _ => assert_eq!(fs::read(f.sidecar()).unwrap(), original),
            }
            let state = saved.state;
            f.reopen();
            assert_eq!(load_review(&f.db, "w", row.id).unwrap().state, state);
        }
    }
}
