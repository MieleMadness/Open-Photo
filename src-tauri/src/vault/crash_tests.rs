//! Cold-process publication acceptance using only disposable synthetic vaults.
use super::*;
const STAGE: &str = "OPENPHOTO_VAULT_TEST_CRASH_STAGE";
const ROOT: &str = "OPENPHOTO_VAULT_TEST_CRASH_ROOT";
const WORKER: &str = "vault::tests::crash::publication_worker";
const EXIT: i32 = 91;

pub(crate) fn point(stage: &str) {
    if std::env::var(STAGE).as_deref() == Ok(stage) && std::env::args().any(|arg| arg == WORKER) {
        // Exit deliberately bypasses Rust destructors without producing a memory dump.
        std::process::exit(EXIT);
    }
}

#[test]
#[ignore = "child process only; the parent supplies a disposable fixture"]
fn publication_worker() {
    let parent = PathBuf::from(std::env::var_os(ROOT).expect("disposable fixture root"));
    let parent = fs::canonicalize(parent).unwrap();
    let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
    assert_eq!(parent.parent(), Some(temp.as_path()));
    assert!(parent
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("openphoto-vault-test-"));
    assert_eq!(
        fs::read(parent.join("crash-fixture-marker")).unwrap(),
        b"synthetic-vault-test-v1"
    );
    let stage = std::env::var(STAGE).unwrap();
    let state = VaultState::new(
        parent.join(if stage.starts_with("restore-") {
            "restored"
        } else {
            "private-vault-v1"
        }),
        Box::new(|| Ok("one".into())),
        || true,
    );
    if stage.starts_with("restore-") {
        state.restore(&parent.join("valid.opvault"), PIN).unwrap();
    } else {
        let grant = state.unlock(PIN).unwrap();
        if stage.starts_with("import-") {
            state
                .import(&grant.token, &parent.join("private holiday.png"))
                .unwrap();
        } else if stage.starts_with("pin-") {
            state
                .change_pin(&grant.token, PIN, "987654", "987654")
                .unwrap();
        } else if stage.starts_with("backup-") {
            state
                .backup(&grant.token, &parent.join("published.opvault"))
                .unwrap();
        } else {
            panic!("unsupported crash stage");
        }
    }
    panic!("requested publication boundary was not reached");
}

#[test]
fn cold_restart_at_publication_boundaries_preserves_complete_state_and_originals() {
    for stage in [
        "import-partial-write",
        "import-object-published",
        "import-catalog-published",
        "pin-before-publish",
        "pin-after-publish",
        "backup-partial-write",
        "backup-before-commit",
        "backup-after-commit",
        "restore-partial-write",
        "restore-before-publish",
        "restore-after-publish",
    ] {
        let (parent, state, _) = fixture();
        fs::write(
            parent.join("crash-fixture-marker"),
            b"synthetic-vault-test-v1",
        )
        .unwrap();
        let source = photo(&parent);
        let original = fs::read(&source).unwrap();
        let grant = state.setup(PIN, PIN).unwrap();
        let baseline = state.import(&grant.token, &source).unwrap();
        state
            .backup(&grant.token, &parent.join("valid.opvault"))
            .unwrap();
        let old_header = fs::read(state.root.join("state.opv")).unwrap();
        drop(state); // Release the real process lease before the child opens storage.
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([WORKER, "--exact", "--ignored", "--test-threads=1"])
            .env(ROOT, &parent)
            .env(STAGE, stage)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("crash worker timed out at {stage}");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(EXIT), "boundary {stage}");
        assert_eq!(fs::read(&source).unwrap(), original, "source at {stage}");
        let reopened = VaultState::new(
            parent.join("private-vault-v1"),
            Box::new(|| Ok("one".into())),
            || true,
        );
        let pin = if stage == "pin-after-publish" {
            "987654"
        } else {
            PIN
        };
        let grant = reopened.unlock(pin).unwrap();
        let expected = if stage == "import-catalog-published" {
            2
        } else {
            1
        };
        assert_eq!(grant.items.len(), expected, "catalog at {stage}");
        let (bytes, ..) = reopened.media(&grant.token, &baseline[0].id, None).unwrap();
        assert_eq!(&*bytes, &original);
        if matches!(
            stage,
            "import-partial-write" | "import-object-published" | "pin-before-publish"
        ) {
            assert_eq!(
                fs::read(reopened.root.join("state.opv")).unwrap(),
                old_header
            );
        }
        if stage.starts_with("pin-") {
            reopened.lock();
            assert!(reopened
                .unlock(if pin == PIN { "987654" } else { PIN })
                .is_err());
        }
        let restored = VaultState::new(
            parent.join("restored"),
            Box::new(|| Ok("one".into())),
            || true,
        );
        if matches!(stage, "restore-partial-write" | "restore-before-publish") {
            assert!(!restored.root.exists());
            restored
                .restore(&parent.join("valid.opvault"), PIN)
                .unwrap();
        } else if stage == "restore-after-publish" {
            let restored_grant = restored.unlock(PIN).unwrap();
            assert_eq!(restored_grant.items.len(), 1);
            let (bytes, ..) = restored
                .media(&restored_grant.token, &baseline[0].id, None)
                .unwrap();
            assert_eq!(&*bytes, &original);
        } else if stage.starts_with("backup-") {
            let backup = parent.join("published.opvault");
            if matches!(stage, "backup-partial-write" | "backup-before-commit") {
                assert!(!backup.exists());
            } else {
                let restored_grant = restored.restore(&backup, PIN).unwrap();
                assert_eq!(restored_grant.items.len(), 1);
                let (bytes, ..) = restored
                    .media(&restored_grant.token, &baseline[0].id, None)
                    .unwrap();
                assert_eq!(&*bytes, &original);
            }
        }
        drop(restored);
        drop(reopened);
        fs::remove_dir_all(parent).unwrap();
    }
}
