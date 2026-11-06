#[cfg(all(test, windows))]
pub(crate) mod step3_tests {
    use super::step2_apply_tests::Fixture;
    use super::*;
    use std::process::{Command, Output, Stdio};
    fn interrupted() -> (Fixture, Review) {
        let mut f = Fixture::new();
        let (row, _) = f.proposal();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "xmp_transactions::step2_apply_tests::abrupt_apply_exit_child",
                "--nocapture",
            ])
            .env("OPENPHOTO_APPLY_CRASH_ROOT", &f.root)
            .env("OPENPHOTO_APPLY_CRASH_BOUNDARY", "after_original_move");
        let output = run(command);
        assert_eq!(output.status.code(), Some(77), "{output:?}");
        f.reopen();
        assert!(!f.sidecar().exists());
        assert_eq!(load_review(&f.db, "w", row.id).unwrap().state, "applying");
        (f, row)
    }
    fn run(mut command: Command) -> Output {
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if std::time::Instant::now() > deadline {
                child.kill().unwrap();
                let output = child.wait_with_output().unwrap();
                panic!("Child process timed out: {output:?}")
            };
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        child.wait_with_output().unwrap()
    }
    pub(crate) fn publication_race_gate() {
        let mut f = Fixture::new();
        let old = fs::read(f.sidecar()).unwrap();
        let (row, view) = f.proposal();
        let result = apply_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, path| {
                if point == "after_original_move" {
                    fs::write(path, b"external-editor-newer-sidecar").unwrap();
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(
            fs::read(f.sidecar()).unwrap(),
            b"external-editor-newer-sidecar"
        );
        let saved = load_review(&f.db, "w", row.id).unwrap();
        assert_eq!(
            fs::read(f.root.join(saved.backup_name.unwrap())).unwrap(),
            old
        );
    }
    #[test]
    fn explicit_restore_returns_exact_original_and_keeps_both_journal_versions() {
        let (mut f, row) = interrupted();
        let current = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(current.can_restore, "{:?}", current.reason);
        let old = row.original.clone().unwrap();
        let restored = restore_on(
            &mut f.db,
            "w",
            row.id,
            &current.fingerprint,
            &row.candidate_hash,
            |_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(restored.state, "restored");
        assert_eq!(fs::read_to_string(f.sidecar()).unwrap(), old);
        assert!(list(&f.db, "w").unwrap().is_empty());
        let saved = load_review(&f.db, "w", row.id).unwrap();
        assert_eq!(saved.candidate, row.candidate);
        assert_eq!(saved.original, row.original);
        assert!(restore_on(
            &mut f.db,
            "w",
            row.id,
            &current.fingerprint,
            &row.candidate_hash,
            |_, _| Ok(())
        )
        .is_err());
    }
    #[test]
    fn restore_refuses_modified_or_replaced_backup_and_unsafe_names() {
        for change in ["bytes", "identity", "name", "alias", "missing"] {
            let (mut f, row) = interrupted();
            let preview = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
            let saved = load_review(&f.db, "w", row.id).unwrap();
            let backup = f.root.join(saved.backup_name.unwrap());
            match change {
                "bytes" => fs::write(&backup, b"external-backup-edit").unwrap(),
                "alias" => fs::hard_link(&backup, f.root.join("backup-alias")).unwrap(),
                "missing" => fs::rename(&backup, backup.with_extension("retained")).unwrap(),
                "identity" => {
                    let bytes = fs::read(&backup).unwrap();
                    fs::rename(&backup, backup.with_extension("retained")).unwrap();
                    fs::write(&backup, bytes).unwrap();
                }
                _ => {
                    f.db.execute(
                        "UPDATE xmp_operations SET backup_name='../photo.jpg' WHERE id=?1",
                        [row.id],
                    )
                    .unwrap();
                }
            }
            assert!(restore_on(
                &mut f.db,
                "w",
                row.id,
                &preview.fingerprint,
                &row.candidate_hash,
                |_, _| Ok(())
            )
            .is_err());
            assert!(!f.sidecar().exists());
            assert!(
                !review_snapshot_on(&mut f.db, "w", row.id)
                    .unwrap()
                    .can_restore
            );
        }
    }
    #[test]
    fn competing_restore_destination_always_wins_and_recovery_stays_intact() {
        let (mut f, row) = interrupted();
        let view = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        let backup = f.root.join(
            load_review(&f.db, "w", row.id)
                .unwrap()
                .backup_name
                .unwrap(),
        );
        let old = fs::read(&backup).unwrap();
        let result = restore_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, path| {
                if point == "before_restore_move" {
                    fs::write(path, b"external-restore-winner").unwrap();
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read(f.sidecar()).unwrap(), b"external-restore-winner");
        assert_eq!(fs::read(backup).unwrap(), old);
        let fresh = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(!fresh.can_restore);
        review_decide_on(&mut f.db, "w", row.id, "kept_external", &fresh.fingerprint).unwrap();
        assert!(list(&f.db, "w").unwrap().is_empty());
    }
    #[test]
    fn restoration_failures_leave_recoverable_states_without_automatic_choices() {
        for boundary in [
            "before_restore_intent_commit",
            "after_restore_intent_commit",
            "before_restore_move",
            "after_restore_move",
            "before_restore_acknowledgement",
        ] {
            let (mut f, row) = interrupted();
            let view = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
            let result = restore_on(
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
            assert!(result.is_err());
            f.reopen();
            let view = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
            assert_eq!(list(&f.db, "w").unwrap().len(), 1);
            if f.sidecar().exists() {
                assert_eq!(
                    fs::read_to_string(f.sidecar()).unwrap(),
                    row.original.clone().unwrap()
                );
                assert!(!view.can_restore);
                review_decide_on(&mut f.db, "w", row.id, "kept_external", &view.fingerprint)
                    .unwrap();
            } else {
                assert!(view.can_restore);
                restore_on(
                    &mut f.db,
                    "w",
                    row.id,
                    &view.fingerprint,
                    &row.candidate_hash,
                    |_, _| Ok(()),
                )
                .unwrap();
            }
            f.reopen();
            assert!(list(&f.db, "w").unwrap().is_empty());
            assert_eq!(
                load_review(&f.db, "w", row.id).unwrap().original,
                row.original
            );
        }
    }
    #[test]
    fn restore_crash_child() {
        let Some(root) = std::env::var_os("OPENPHOTO_RESTORE_CRASH_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let mut db = Connection::open(root.join("user-data.sqlite")).unwrap();
        schema(&db).unwrap();
        let row = list(&db, "w").unwrap().remove(0);
        let view = review_snapshot_on(&mut db, "w", row.id).unwrap();
        let boundary = std::env::var("OPENPHOTO_RESTORE_CRASH_BOUNDARY").unwrap();
        let _ = restore_on(
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
        panic!("Restore crash boundary not reached")
    }
    #[test]
    fn cold_restart_after_restore_crashes_requires_a_fresh_explicit_decision() {
        for boundary in [
            "before_restore_intent_commit",
            "after_restore_intent_commit",
            "before_restore_move",
            "after_restore_move",
            "before_restore_acknowledgement",
        ] {
            let (mut f, row) = interrupted();
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "xmp_transactions::step3_tests::restore_crash_child",
                    "--nocapture",
                ])
                .env("OPENPHOTO_RESTORE_CRASH_ROOT", &f.root)
                .env("OPENPHOTO_RESTORE_CRASH_BOUNDARY", boundary);
            let output = run(command);
            assert_eq!(output.status.code(), Some(77), "{output:?}");
            f.reopen();
            let first = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
            let state = load_review(&f.db, "w", row.id).unwrap().state;
            f.reopen();
            assert_eq!(load_review(&f.db, "w", row.id).unwrap().state, state);
            let second = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
            assert_eq!(first.fingerprint, second.fingerprint);
            if second.current.is_some() {
                assert_eq!(second.current, row.original);
                review_decide_on(&mut f.db, "w", row.id, "kept_external", &second.fingerprint)
                    .unwrap();
            } else {
                assert!(second.can_restore);
                restore_on(
                    &mut f.db,
                    "w",
                    row.id,
                    &second.fingerprint,
                    &row.candidate_hash,
                    |_, _| Ok(()),
                )
                .unwrap();
            }
            assert!(list(&f.db, "w").unwrap().is_empty());
        }
    }
    fn exiftool(args: &[&str], path: &Path) -> Output {
        let mut command = if let Some(exe) = std::env::var_os("OPENPHOTO_EXIFTOOL_BIN") {
            Command::new(exe)
        } else {
            let mut c = Command::new(
                std::env::var_os("OPENPHOTO_EXIFTOOL_PERL")
                    .expect("Set OPENPHOTO_EXIFTOOL_PERL or OPENPHOTO_EXIFTOOL_BIN"),
            );
            c.arg(
                std::env::var_os("OPENPHOTO_EXIFTOOL_SCRIPT")
                    .expect("Set OPENPHOTO_EXIFTOOL_SCRIPT"),
            );
            c
        };
        let portable = path.to_string_lossy().replace('\\', "/");
        command
            // Do not load user-specific ExifTool configuration during trials.
            .args(["-config", ""])
            .args(args.iter().map(|a| a.strip_prefix("//?/").unwrap_or(a)))
            .arg(portable.strip_prefix("//?/").unwrap_or(&portable));
        run(command)
    }
    #[test]
    #[ignore = "Explicit third-party editor compatibility trial; requires a verified local ExifTool runtime"]
    fn exiftool_editor_matrix_preserves_winning_edits_and_originals() {
        for flag in ["-overwrite_original", "-overwrite_original_in_place"] {
            let mut f = Fixture::new();
            let (row, view) = f.proposal();
            let output = exiftool(
                &[flag, "-XMP-dc:Subject+=ExternalBeforeApply"],
                &f.sidecar(),
            );
            assert!(output.status.success(), "{output:?}");
            let external = fs::read(f.sidecar()).unwrap();
            assert!(String::from_utf8_lossy(&external).contains("ExternalBeforeApply"));
            assert!(f.apply(&row, &view).is_err());
            assert_eq!(fs::read(f.sidecar()).unwrap(), external);
            let mut f = Fixture::new();
            let old = fs::read(f.sidecar()).unwrap();
            let (row, view) = f.proposal();
            let result = apply_on(
                &mut f.db,
                "w",
                row.id,
                &view.fingerprint,
                &row.candidate_hash,
                |point, path| {
                    if point == "before_original_move" {
                        let output = exiftool(&[flag, "-XMP-dc:Subject+=BlockedEditor"], path);
                        eprintln!("ExifTool protected-sidecar {flag}: {output:?}");
                        assert!(
                            !output.status.success(),
                            "Editor reported success under the retained guard"
                        );
                        assert_eq!(fs::read(path).unwrap(), old);
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                fs::read(f.root.join(result.backup_name.unwrap())).unwrap(),
                old
            );
            assert!(!fs::read_to_string(f.sidecar())
                .unwrap()
                .contains("BlockedEditor"));
        }
        let mut f = Fixture::new();
        let template = f.root.join("external-template.xmp");
        fs::write(
            &template,
            crate::render_openphoto_xmp(&["ExternalTemplate".into()]),
        )
        .unwrap();
        let original = fs::read(f.sidecar()).unwrap();
        let (row, view) = f.proposal();
        let mut external = Vec::new();
        let result = apply_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, path| {
                if point == "after_original_move" {
                    let out = exiftool(
                        &[
                            "-o",
                            &path.to_string_lossy().replace('\\', "/"),
                            "-XMP-dc:Subject=ExternalWinner",
                        ],
                        &template,
                    );
                    assert!(out.status.success(), "{out:?}");
                    external = fs::read(path).unwrap();
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read(f.sidecar()).unwrap(), external);
        assert!(String::from_utf8_lossy(&external).contains("ExternalWinner"));
        let saved = load_review(&f.db, "w", row.id).unwrap();
        assert_eq!(
            fs::read(f.root.join(saved.backup_name.unwrap())).unwrap(),
            original
        );
        let mut f = Fixture::new();
        let (row, view) = f.proposal();
        let mut latest = Vec::new();
        apply_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, path| {
                if point == "after_apply_publish" {
                    let out = exiftool(
                        &["-overwrite_original", "-XMP-dc:Subject+=LaterExternalEdit"],
                        path,
                    );
                    assert!(out.status.success(), "{out:?}");
                    latest = fs::read(path).unwrap();
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(fs::read(f.sidecar()).unwrap(), latest);
        assert!(String::from_utf8_lossy(&latest).contains("LaterExternalEdit"));
        assert_eq!(
            review_snapshot_on(&mut f.db, "w", row.id)
                .unwrap()
                .observation,
            "changed"
        );
        let (mut f, row) = interrupted();
        let template = f.root.join("recovery-editor-template.xmp");
        fs::write(
            &template,
            crate::render_openphoto_xmp(&["RecoveryEditor".into()]),
        )
        .unwrap();
        let view = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        assert!(view.can_restore);
        let backup = f.root.join(
            load_review(&f.db, "w", row.id)
                .unwrap()
                .backup_name
                .unwrap(),
        );
        let original = fs::read(&backup).unwrap();
        let mut winning_bytes = Vec::new();
        let result = restore_on(
            &mut f.db,
            "w",
            row.id,
            &view.fingerprint,
            &row.candidate_hash,
            |point, path| {
                if point == "before_restore_move" {
                    let out = exiftool(
                        &[
                            "-o",
                            &path.to_string_lossy().replace('\\', "/"),
                            "-XMP-dc:Subject=ExternalRecoveryWinner",
                        ],
                        &template,
                    );
                    assert!(out.status.success(), "{out:?}");
                    winning_bytes = fs::read(path).unwrap();
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read(f.sidecar()).unwrap(), winning_bytes);
        assert_eq!(fs::read(&backup).unwrap(), original);
        let fresh = review_snapshot_on(&mut f.db, "w", row.id).unwrap();
        review_decide_on(&mut f.db, "w", row.id, "kept_external", &fresh.fingerprint).unwrap();
        assert_eq!(fs::read(f.sidecar()).unwrap(), winning_bytes);
        assert_eq!(fs::read(backup).unwrap(), original);
        eprintln!("Real ExifTool editor matrix passed: pre-Apply edits, guarded replacement/in-place edits, Apply-gap winner, post-publication edit, and explicit-restore winner. Every fixture checked original media bytes in Drop.");
    }
}
