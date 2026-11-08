use super::*;
const PIN: &str = "123456";
fn fixture() -> (PathBuf, Arc<VaultState>, Arc<Mutex<String>>) {
    let parent =
        std::env::temp_dir().join(format!("openphoto-vault-test-{}", crypto::id().unwrap()));
    fs::create_dir(&parent).unwrap();
    let workspace = Arc::new(Mutex::new("one".to_string()));
    let owner = workspace.clone();
    let state = Arc::new(VaultState::new(
        parent.join("private-vault-v1"),
        Box::new(move || Ok(owner.lock().unwrap().clone())),
        || true,
    ));
    (parent, state, workspace)
}
fn photo(parent: &Path) -> PathBuf {
    let path = parent.join("private holiday.png");
    image::RgbImage::from_pixel(24, 16, image::Rgb([10, 20, 30]))
        .save(&path)
        .unwrap();
    path
}
#[test]
fn setup_import_ranges_lock_and_restart_keep_only_ciphertext_and_preserve_originals() {
    let (parent, state, _) = fixture();
    assert!(!state.status().unwrap().configured);
    assert!(state.list(&"0".repeat(64)).is_err());
    assert!(state.setup("123", "123").is_err());
    let grant = state.setup(PIN, PIN).unwrap();
    assert!(state.setup(PIN, PIN).is_err());
    let source = photo(&parent);
    let original = fs::read(&source).unwrap();
    let items = state.import(&grant.token, &source).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "private holiday.png");
    let id = items[0].id.clone();
    let (bytes, mime, _, _, total, _) = state.media(&grant.token, &id, None).unwrap();
    assert_eq!(&*bytes, &original);
    assert_eq!(mime, "image/png");
    assert_eq!(total, original.len() as u64);
    let (range, _, start, end, _, partial) =
        state.media(&grant.token, &id, Some("bytes=3-15")).unwrap();
    assert_eq!(&*range, &original[3..16]);
    assert_eq!((start, end, partial), (3, 15, true));
    let raw = fs::read(state.root.join("state.opv")).unwrap();
    assert!(!String::from_utf8_lossy(&raw).contains("private holiday.png"));
    assert!(!raw.windows(6).any(|w| w == PIN.as_bytes()));
    let encrypted = fs::read(state.root.join(format!("{id}.opv"))).unwrap();
    assert!(!encrypted.windows(8).any(|w| w == &original[..8]));
    assert!(state.store().unwrap().object_path("../escape").is_err());
    state.lock();
    assert!(state.media(&grant.token, &id, None).is_err());
    assert!(state.import(&grant.token, &source).is_err());
    drop(state);
    let state = VaultState::new(
        parent.join("private-vault-v1"),
        Box::new(|| Ok("one".into())),
        || true,
    );
    let reopened = state.unlock(PIN).unwrap();
    assert_ne!(reopened.token, grant.token);
    assert_eq!(reopened.items[0].name, "private holiday.png");
    assert_eq!(fs::read(source).unwrap(), original);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn wrong_pin_backoff_persists_restart_and_success_resets_it() {
    let (parent, mut state, _) = fixture();
    Arc::get_mut(&mut state).unwrap().clock = Box::new(|| 1000);
    state.setup(PIN, PIN).unwrap();
    state.lock();
    for _ in 0..5 {
        assert!(state.unlock("654321").is_err());
    }
    let attempts = state.attempts().unwrap();
    assert_eq!(attempts.failures, 5);
    assert!(attempts.until > 1000);
    drop(state);
    let mut state = VaultState::new(
        parent.join("private-vault-v1"),
        Box::new(|| Ok("one".into())),
        || true,
    );
    state.clock = Box::new(|| 1000);
    assert_eq!(state.attempts().unwrap().failures, 5);
    assert!(state.unlock(PIN).is_err());
    state
        .write_attempts(&Attempts {
            version: 1,
            failures: 5,
            until: 0,
        })
        .unwrap();
    state.unlock(PIN).unwrap();
    assert_eq!(state.attempts().unwrap().failures, 0);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn pin_change_rewraps_atomically_invalidates_old_tokens_and_keeps_media() {
    let (parent, state, _) = fixture();
    let first = state.setup(PIN, PIN).unwrap();
    state.import(&first.token, &photo(&parent)).unwrap();
    assert!(state
        .change_pin(&first.token, PIN, "new passphrase 123", "does not match")
        .is_err());
    let next = state
        .change_pin(
            &first.token,
            PIN,
            "new passphrase 123",
            "new passphrase 123",
        )
        .unwrap();
    assert_ne!(next.token, first.token);
    assert!(state.list(&first.token).is_err());
    state.lock();
    assert!(state.unlock(PIN).is_err());
    let next = state.unlock("new passphrase 123").unwrap();
    assert_eq!(next.items.len(), 1);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn tampered_unknown_and_racing_state_are_preserved_not_replaced() {
    let (parent, state, _) = fixture();
    let grant = state.setup(PIN, PIN).unwrap();
    let source = photo(&parent);
    let items = state.import(&grant.token, &source).unwrap();
    let object = state.store().unwrap().object_path(&items[0].id).unwrap();
    let mut raw = fs::read(&object).unwrap();
    raw[10] ^= 1;
    fs::write(&object, &raw).unwrap();
    assert!(state.media(&grant.token, &items[0].id, None).is_err());
    assert!(state
        .backup(&grant.token, &parent.join("bad.opvault"))
        .is_err());
    assert!(!parent.join("bad.opvault").exists());
    let (header, before) = state.store().unwrap().state().unwrap();
    let mut future: serde_json::Value = serde_json::from_slice(&before).unwrap();
    future["version"] = serde_json::json!(99);
    let future = serde_json::to_vec(&future).unwrap();
    fs::write(state.root.join("state.opv"), &future).unwrap();
    assert!(state.status().is_err());
    assert!(state.unlock(PIN).is_err());
    assert!(state.import(&grant.token, &source).is_err());
    assert!(state
        .store()
        .unwrap()
        .publish(&before, &header, || false)
        .is_err());
    assert_eq!(fs::read(state.root.join("state.opv")).unwrap(), future);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn backup_restore_verifies_all_chunks_refuses_existing_vault_and_tampering() {
    let (parent, state, _) = fixture();
    let grant = state.setup(PIN, PIN).unwrap();
    let source = parent.join("private clip.mp4");
    let original = (0..(CHUNK as usize + 8192))
        .map(|i| (i % 251) as u8)
        .collect::<Vec<_>>();
    fs::write(&source, &original).unwrap();
    let items = state.import(&grant.token, &source).unwrap();
    let backup = parent.join("archive.opvault");
    state.backup(&grant.token, &backup).unwrap();
    let saved = fs::read(&backup).unwrap();
    assert!(state.backup(&grant.token, &backup).is_err());
    assert_eq!(fs::read(&backup).unwrap(), saved);
    assert!(state.restore(&backup, PIN).is_err());
    let restore_parent = parent.join("restore");
    fs::create_dir(&restore_parent).unwrap();
    let restored = VaultState::new(
        restore_parent.join("private-vault-v1"),
        Box::new(|| Ok("one".into())),
        || true,
    );
    let unlocked = restored.restore(&backup, PIN).unwrap();
    let (range, _, start, end, _, _) = restored
        .media(
            &unlocked.token,
            &items[0].id,
            Some(&format!("bytes={}-{}", CHUNK - 5, CHUNK + 5)),
        )
        .unwrap();
    assert_eq!((start, end), (CHUNK - 5, CHUNK + 5));
    assert_eq!(
        &*range,
        &original[(CHUNK - 5) as usize..=(CHUNK + 5) as usize]
    );
    assert!(restored.restore(&backup, PIN).is_err());
    drop(restored);
    let damaged = parent.join("tampered.opvault");
    let mut changed = saved;
    let last = changed.len() - 1;
    changed[last] ^= 1;
    fs::write(&damaged, changed).unwrap();
    let bad_parent = parent.join("bad-restore");
    fs::create_dir(&bad_parent).unwrap();
    let bad = VaultState::new(
        bad_parent.join("private-vault-v1"),
        Box::new(|| Ok("one".into())),
        || true,
    );
    assert!(bad.restore(&damaged, PIN).is_err());
    assert!(!bad.root.exists());
    assert_eq!(fs::read_dir(&bad_parent).unwrap().count(), 1);
    assert_eq!(fs::read(source).unwrap(), original);
    drop(bad);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn native_idle_and_workspace_changes_revoke_direct_requests() {
    let (parent, state, workspace) = fixture();
    let first = state.setup(PIN, PIN).unwrap();
    state
        .session
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .last_activity = Instant::now() - IDLE;
    assert!(state.watchdog());
    assert!(state.list(&first.token).is_err());
    let next = state.unlock(PIN).unwrap();
    *workspace.lock().unwrap() = "two".into();
    assert!(state.watchdog());
    assert!(state.list(&next.token).is_err());
    assert!(!state.watchdog());
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn lock_interrupts_import_before_publication_and_source_writer_and_hardlinks_are_rejected() {
    let (parent, state, _) = fixture();
    let grant = state.setup(PIN, PIN).unwrap();
    let source = photo(&parent);
    let original = fs::read(&source).unwrap();
    let writer = OpenOptions::new().write(true).open(&source).unwrap();
    assert!(state.import(&grant.token, &source).is_err());
    drop(writer);
    let linked = parent.join("linked.png");
    fs::hard_link(&source, &linked).unwrap();
    assert!(state.import(&grant.token, &linked).is_err());
    fs::remove_file(linked).unwrap();
    let store = state.store().unwrap();
    let (header, _) = store.state().unwrap();
    let object = Object {
        id: crypto::id().unwrap(),
        name: "test".into(),
        mime: "video/mp4".into(),
        bytes: original.len() as u64,
        digest: format!("{:x}", Sha256::digest(&original)),
        key: crypto::random::<32>().unwrap().to_vec(),
    };
    let destination = store.object_path(&object.id).unwrap();
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .unwrap();
    assert!(encrypt_file(
        &mut regular(&source, false).unwrap(),
        &mut file,
        &header.id,
        &object,
        &|| true
    )
    .is_err());
    drop(file);
    fs::remove_file(destination).unwrap();
    state.lock();
    assert!(state.import(&grant.token, &source).is_err());
    assert_eq!(fs::read(source).unwrap(), original);
    drop(store);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn media_ranges_are_bounded_and_reject_malformed_or_exclusive_end() {
    assert_eq!(
        transfer::media_range(Some("bytes=-5"), 10, false).unwrap(),
        (5, 9, true)
    );
    assert_eq!(
        transfer::media_range(Some("bytes=3-"), 10, false).unwrap(),
        (3, 9, true)
    );
    assert_eq!(
        transfer::media_range(Some("bytes=0-"), MAX_VIDEO, true).unwrap(),
        (0, 8 * 1024 * 1024 - 1, true)
    );
    for range in [
        "bytes=10-",
        "bytes=5-2",
        "bytes=-0",
        "bytes=0-1,3-4",
        "bytes=abc-",
        "items=0-1",
    ] {
        assert!(transfer::media_range(Some(range), 10, false).is_err());
    }
}
#[test]
fn process_lease_refuses_a_second_writer_without_touching_stored_data() {
    let (parent, state, _) = fixture();
    state.setup(PIN, PIN).unwrap();
    let before = fs::read(state.root.join("state.opv")).unwrap();
    let other = VaultState::new(state.root.clone(), Box::new(|| Ok("one".into())), || true);
    assert!(other.status().is_err());
    assert!(other.unlock(PIN).is_err());
    assert_eq!(fs::read(state.root.join("state.opv")).unwrap(), before);
    drop(other);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn locking_a_running_import_discards_unpublished_work_and_keeps_the_original() {
    let (parent, state, _) = fixture();
    state.setup(PIN, PIN).unwrap();
    let source = parent.join("running.mp4");
    let original = vec![19; CHUNK as usize * 2 + 10];
    fs::write(&source, &original).unwrap();
    state.lock();
    drop(state);
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (continue_tx, continue_rx) = std::sync::mpsc::channel();
    let continue_rx = Mutex::new(continue_rx);
    let remaining = Arc::new(AtomicU64::new(u64::MAX));
    let counter = remaining.clone();
    let state = Arc::new(VaultState::new(
        parent.join("private-vault-v1"),
        Box::new(move || {
            if counter.fetch_sub(1, Ordering::SeqCst) == 1 {
                entered_tx.send(()).unwrap();
                continue_rx.lock().unwrap().recv().unwrap();
            }
            Ok("one".into())
        }),
        || true,
    ));
    let grant = state.unlock(PIN).unwrap();
    remaining.store(4, Ordering::SeqCst);
    let worker = state.clone();
    let path = source.clone();
    let token = grant.token.clone();
    let join = std::thread::spawn(move || worker.import(&token, &path));
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(OpenOptions::new().write(true).open(&source).is_err());
    state.lock();
    continue_tx.send(()).unwrap();
    assert!(join.join().unwrap().is_err());
    let next = state.unlock(PIN).unwrap();
    assert!(next.items.is_empty());
    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(fs::read_dir(&state.root).unwrap().count(), 2);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn unavailable_or_locked_windows_session_revokes_keys_and_never_serves_media() {
    let (parent, mut state, _) = fixture();
    let unlocked = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let signal = unlocked.clone();
    Arc::get_mut(&mut state).unwrap().screen = Box::new(move || signal.load(Ordering::SeqCst));
    let grant = state.setup(PIN, PIN).unwrap();
    let items = state.import(&grant.token, &photo(&parent)).unwrap();
    unlocked.store(false, Ordering::SeqCst);
    assert!(state.watchdog());
    assert!(state.session.lock().unwrap().is_none());
    assert!(state.media(&grant.token, &items[0].id, None).is_err());
    assert!(state.unlock(PIN).is_err());
    unlocked.store(true, Ordering::SeqCst);
    state.unlock(PIN).unwrap();
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn replacing_ciphertext_with_another_valid_item_never_returns_the_wrong_plaintext() {
    let (parent, state, _) = fixture();
    let grant = state.setup(PIN, PIN).unwrap();
    let first = photo(&parent);
    let second = parent.join("another private.png");
    fs::copy(&first, &second).unwrap();
    state.import(&grant.token, &first).unwrap();
    let items = state.import(&grant.token, &second).unwrap();
    let store = state.store().unwrap();
    fs::copy(
        store.object_path(&items[1].id).unwrap(),
        store.object_path(&items[0].id).unwrap(),
    )
    .unwrap();
    assert!(state.media(&grant.token, &items[0].id, None).is_err());
    assert!(state.media(&grant.token, &items[1].id, None).is_ok());
    drop(store);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn directory_junctions_and_unknown_attempt_formats_are_not_followed_or_repaired() {
    let (parent, state, _) = fixture();
    let target = parent.join("outside");
    fs::create_dir(&target).unwrap();
    use std::os::windows::process::CommandExt;
    let output = std::process::Command::new("cmd.exe")
        .args(["/c", "mklink", "/J"])
        .arg(&state.root)
        .arg(&target)
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(state.status().is_err());
    assert!(state.setup(PIN, PIN).is_err());
    assert!(fs::read_dir(&target).unwrap().next().is_none());
    fs::remove_dir(&state.root).unwrap();
    let future = br#"{"version":99,"failures":0,"until":0}"#;
    fs::write(parent.join("vault-attempts-v1.json"), future).unwrap();
    assert!(state.status().is_err());
    assert!(state.setup(PIN, PIN).is_err());
    assert_eq!(
        fs::read(parent.join("vault-attempts-v1.json")).unwrap(),
        future
    );
    assert!(!state.root.exists());
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn failed_closed_file_verification_cannot_publish_and_success_preserves_no_overwrite() {
    let (parent, state, _) = fixture();
    let destination = parent.join("verified.opv");
    let mut output = crate::safe_output::StagedOutput::new(&destination).unwrap();
    output
        .file()
        .write_all(b"opaque ciphertext fixture")
        .unwrap();
    assert!(output
        .verify_closed(|_| Err("injected verification failure".into()))
        .is_err());
    assert!(output.commit().is_err());
    assert!(!destination.exists());
    let mut output = crate::safe_output::StagedOutput::new(&destination).unwrap();
    output
        .file()
        .write_all(b"opaque ciphertext fixture")
        .unwrap();
    output
        .verify_closed(|path| {
            assert_eq!(fs::read(path).unwrap(), b"opaque ciphertext fixture");
            Ok(())
        })
        .unwrap();
    fs::write(&destination, b"racing existing output").unwrap();
    assert!(output.commit().is_err());
    assert_eq!(fs::read(&destination).unwrap(), b"racing existing output");
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn authenticated_protocol_has_no_store_range_headers_and_revokes_old_urls() {
    let (parent, state, _) = fixture();
    let grant = state.setup(PIN, PIN).unwrap();
    let source = photo(&parent);
    let original = fs::read(&source).unwrap();
    let items = state.import(&grant.token, &source).unwrap();
    let url = format!("http://vault.localhost/{}/{}", grant.token, items[0].id);
    let request = |method: &str, range: &str, origin: &str| {
        tauri::http::Request::builder()
            .method(method)
            .uri(&url)
            .header("Origin", origin)
            .header("Range", range)
            .body(vec![])
            .unwrap()
    };
    let response = transfer::response(
        &state,
        request("GET", "bytes=2-9", "http://tauri.localhost"),
    );
    if !available() {
        assert_eq!(response.status(), 404);
        drop(state);
        fs::remove_dir_all(parent).unwrap();
        return;
    }
    assert_eq!(response.status(), 206);
    assert_eq!(response.headers()["Cache-Control"], "no-store, max-age=0");
    assert_eq!(response.headers()["Referrer-Policy"], "no-referrer");
    assert_eq!(response.headers()["Content-Type"], "image/png");
    assert_eq!(response.body(), &original[2..10]);
    let invalid = transfer::response(
        &state,
        request("GET", "bytes=999999-", "http://tauri.localhost"),
    );
    assert_eq!(invalid.status(), 416);
    assert_eq!(
        invalid.headers()["Content-Range"],
        format!("bytes */{}", original.len())
    );
    assert_eq!(
        transfer::response(
            &state,
            request("GET", "bytes=0-10", "https://untrusted.example")
        )
        .status(),
        404
    );
    let head = transfer::response(
        &state,
        request("HEAD", "bytes=0-10", "http://tauri.localhost"),
    );
    assert_eq!(head.status(), 206);
    assert!(head.body().is_empty());
    assert_eq!(head.headers()["Content-Length"], "11");
    state.lock();
    let replay = transfer::response(
        &state,
        request("GET", "bytes=0-10", "http://tauri.localhost"),
    );
    assert_eq!(replay.status(), 404);
    assert!(replay.body().is_empty());
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn complete_vault_publication_never_replaces_a_racing_destination() {
    let (parent, state, _) = fixture();
    let staged = staged_directory(&parent).unwrap();
    fs::write(staged.0.join("state.opv"), b"encrypted fixture state").unwrap();
    fs::create_dir(&state.root).unwrap();
    fs::write(state.root.join("preserve.opv"), b"existing vault fixture").unwrap();
    assert!(publish_directory(&staged, &state.root, || false).is_err());
    assert_eq!(
        fs::read(state.root.join("preserve.opv")).unwrap(),
        b"existing vault fixture"
    );
    assert!(staged.0.join("state.opv").exists());
    drop(staged);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn poisoned_session_is_revoked_and_cleared_without_resetting_encrypted_storage() {
    let (parent, state, _) = fixture();
    let grant = state.setup(PIN, PIN).unwrap();
    let before = fs::read(state.root.join("state.opv")).unwrap();
    let worker = state.clone();
    assert!(std::thread::spawn(move || {
        let _guard = worker.session.lock().unwrap();
        panic!("injected vault session failure");
    })
    .join()
    .is_err());
    assert!(state.watchdog());
    let slot = state
        .session
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    assert!(slot.is_none());
    drop(slot);
    assert!(state.list(&grant.token).is_err());
    assert_eq!(fs::read(state.root.join("state.opv")).unwrap(), before);
    drop(state);
    let restarted = VaultState::new(
        parent.join("private-vault-v1"),
        Box::new(|| Ok("one".into())),
        || true,
    );
    restarted.unlock(PIN).unwrap();
    drop(restarted);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn concurrent_metadata_snapshots_and_publication_preserve_complete_versions_and_media_locks() {
    let (parent, state, _) = fixture();
    let path = parent.join("metadata.opv");
    let first = vec![17; 4096];
    let second = vec![29; 4096];
    fs::write(&path, &first).unwrap();
    let start = Arc::new(std::sync::Barrier::new(5));
    let mut readers = vec![];
    for _ in 0..4 {
        let path = path.clone();
        let start = start.clone();
        let first = first.clone();
        let second = second.clone();
        readers.push(std::thread::spawn(move || {
            start.wait();
            for _ in 0..50 {
                let snapshot = read_bounded(&path, 4096).unwrap();
                assert!(snapshot == first || snapshot == second);
            }
        }));
    }
    start.wait();
    for index in 0..25 {
        replace(&path, if index % 2 == 0 { &second } else { &first }).unwrap();
    }
    for reader in readers {
        reader.join().unwrap();
    }
    let snapshot = read_bounded(&path, 4096).unwrap();
    replace(&path, &first).unwrap();
    assert_eq!(snapshot, second);
    let media = regular(&path, false).unwrap();
    assert!(fs::rename(&path, parent.join("moved.opv")).is_err());
    assert!(OpenOptions::new().write(true).open(&path).is_err());
    drop(media);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn atomic_publication_retries_only_bounded_windows_conditions_and_observes_cancellation() {
    let mut calls = 0;
    assert!(atomic_until(&|| false, || {
        calls += 1;
        if calls < 3 {
            Err(std::io::Error::from_raw_os_error(5))
        } else {
            Ok(())
        }
    })
    .is_ok());
    assert_eq!(calls, 3);
    let mut calls = 0;
    assert!(atomic_until(&|| false, || {
        calls += 1;
        Err::<(), _>(std::io::Error::from_raw_os_error(183))
    })
    .is_err());
    assert_eq!(calls, 1);
    let mut calls = 0;
    assert!(atomic_until(&|| false, || {
        calls += 1;
        Err::<(), _>(std::io::Error::from_raw_os_error(32))
    })
    .is_err());
    assert_eq!(calls, 7);
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let mut calls = 0;
    let result = atomic_until(&|| cancelled.load(Ordering::SeqCst), || {
        calls += 1;
        cancelled.store(true, Ordering::SeqCst);
        Err::<(), _>(std::io::Error::from_raw_os_error(5))
    });
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(calls, 1);
}

#[test]
fn queued_authentication_cannot_cross_a_lock_generation() {
    let (parent, state, _) = fixture();
    let expected = state.epoch.load(Ordering::SeqCst);
    let gate = state.operation.lock().unwrap();
    let worker = state.clone();
    let queued = std::thread::spawn(move || worker.setup_at(PIN, PIN, expected));
    state.lock();
    drop(gate);
    assert!(queued.join().unwrap().is_err());
    assert!(!state.root.exists());
    state.setup(PIN, PIN).unwrap();
    let expected = state.epoch.load(Ordering::SeqCst);
    let gate = state.operation.lock().unwrap();
    let worker = state.clone();
    let queued = std::thread::spawn(move || worker.unlock_at(PIN, expected));
    state.lock();
    drop(gate);
    assert!(queued.join().unwrap().is_err());
    assert!(state.session.lock().unwrap().is_none());
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
#[test]
fn obsolete_revocations_cannot_remove_a_replacement_session() {
    let (parent, state, _) = fixture();
    let old = state.setup(PIN, PIN).unwrap();
    let notice = state.lock_notice();
    let fresh = state.unlock(PIN).unwrap();
    assert!(fresh.epoch > notice.through_epoch);
    assert!(state.revoke_if(old.epoch).is_none());
    assert!(state.revoke_token(&old.token).is_none());
    assert!(state.list(&fresh.token).is_ok());
    assert_eq!(
        state.revoke_token(&fresh.token).unwrap().through_epoch,
        fresh.epoch
    );
    assert!(state.list(&fresh.token).is_err());
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn delayed_workspace_validation_cannot_revoke_a_new_session() {
    for mode in 0..3 {
        let (parent, mut state, _) = fixture();
        let armed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let pause = armed.clone();
        let (entered, wait) = std::sync::mpsc::channel();
        let (resume, resumed) = std::sync::mpsc::channel();
        let resumed = Mutex::new(resumed);
        Arc::get_mut(&mut state).unwrap().authority = Box::new(move || {
            if pause.swap(false, Ordering::SeqCst) {
                entered.send(()).unwrap();
                resumed.lock().unwrap().recv().unwrap();
                return Ok("obsolete-workspace".into());
            }
            Ok("one".into())
        });
        let old = state.setup(PIN, PIN).unwrap();
        let lease = state.lease(&old.token, false).unwrap();
        armed.store(true, Ordering::SeqCst);
        let worker = state.clone();
        let observer = std::thread::spawn(move || match mode {
            0 => assert!(!worker.current(&lease, false)),
            1 => assert!(worker.lease(&old.token, false).is_err()),
            _ => assert!(worker.watchdog_notice().is_none()),
        });
        wait.recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        state.lock();
        let fresh = state.unlock(PIN).unwrap();
        resume.send(()).unwrap();
        observer.join().unwrap();
        assert!(state.list(&fresh.token).is_ok());
        drop(state);
        fs::remove_dir_all(parent).unwrap();
    }
}

#[test]
fn queued_restore_cannot_publish_after_lock() {
    let (parent, source_state, _) = fixture();
    let grant = source_state.setup(PIN, PIN).unwrap();
    let source = parent.join("valid.opvault");
    source_state.backup(&grant.token, &source).unwrap();
    let state = Arc::new(VaultState::new(
        parent.join("restored"),
        Box::new(|| Ok("one".into())),
        || true,
    ));
    let expected = state.epoch.load(Ordering::SeqCst);
    let gate = state.operation.lock().unwrap();
    let worker = state.clone();
    let queued = std::thread::spawn(move || worker.restore_at(&source, PIN, expected));
    state.lock();
    drop(gate);
    assert!(queued.join().unwrap().is_err());
    assert!(!state.root.exists());
    assert!(state.session.lock().unwrap().is_none());
    drop(state);
    drop(source_state);
    fs::remove_dir_all(parent).unwrap();
}

#[path = "crash_tests.rs"]
pub(super) mod crash;

#[test]
fn persistent_metadata_sharing_denial_preserves_catalog_pin_and_source() {
    let (parent, state, _) = fixture();
    let grant = state.setup(PIN, PIN).unwrap();
    let source = photo(&parent);
    let original = fs::read(&source).unwrap();
    let header_path = state.root.join("state.opv");
    let before = fs::read(&header_path).unwrap();
    let held = regular(&header_path, false).unwrap();
    assert!(state.import(&grant.token, &source).is_err());
    assert_eq!(fs::read(&header_path).unwrap(), before);
    assert!(state.list(&grant.token).unwrap().is_empty());
    assert_eq!(fs::read(&source).unwrap(), original);
    assert!(state
        .change_pin(&grant.token, PIN, "987654", "987654")
        .is_err());
    assert_eq!(fs::read(&header_path).unwrap(), before);
    assert_eq!(fs::read_dir(&state.root).unwrap().count(), 2); // state + process lease, no orphan from handled failure.
    drop(held);
    let items = state.import(&grant.token, &source).unwrap();
    assert_eq!(items.len(), 1);
    state.lock();
    assert!(state.unlock(PIN).is_ok());
    assert_eq!(fs::read(source).unwrap(), original);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn image_headers_determine_private_mime_and_unsupported_disguised_formats_are_refused() {
    let (parent, state, _) = fixture();
    let grant = state.setup(PIN, PIN).unwrap();
    let image = image::RgbImage::from_pixel(24, 16, image::Rgb([10, 20, 30]));
    for (name, format, mime) in [
        ("actually-png.jpg", image::ImageFormat::Png, "image/png"),
        ("actually-jpeg.png", image::ImageFormat::Jpeg, "image/jpeg"),
        ("actually-bmp.webp", image::ImageFormat::Bmp, "image/bmp"),
    ] {
        let source = parent.join(name);
        image.save_with_format(&source, format).unwrap();
        let original = fs::read(&source).unwrap();
        let items = state.import(&grant.token, &source).unwrap();
        let item = items.last().unwrap();
        assert_eq!(item.name, name);
        assert_eq!(item.mime, mime);
        let (bytes, actual, ..) = state.media(&grant.token, &item.id, None).unwrap();
        assert_eq!(actual, mime);
        assert_eq!(&*bytes, &original);
        if available() {
            let response = transfer::response(
                &state,
                tauri::http::Request::builder()
                    .uri(format!(
                        "http://vault.localhost/{}/{}",
                        grant.token, item.id
                    ))
                    .body(vec![])
                    .unwrap(),
            );
            assert_eq!(response.status(), 200);
            assert_eq!(response.headers()["Content-Type"], mime);
            assert_eq!(response.body(), &original);
        }
        assert_eq!(fs::read(source).unwrap(), original);
    }
    let before = fs::read(state.root.join("state.opv")).unwrap();
    let files_before = fs::read_dir(&state.root).unwrap().count();
    let disguised = parent.join("unsupported-tiff.png");
    image
        .save_with_format(&disguised, image::ImageFormat::Tiff)
        .unwrap();
    let original = fs::read(&disguised).unwrap();
    assert!(state.import(&grant.token, &disguised).is_err());
    let malformed = parent.join("not-an-image.jpg");
    fs::write(&malformed, b"invalid image fixture").unwrap();
    assert!(state.import(&grant.token, &malformed).is_err());
    assert_eq!(fs::read(state.root.join("state.opv")).unwrap(), before);
    assert_eq!(fs::read_dir(&state.root).unwrap().count(), files_before);
    assert_eq!(fs::read(disguised).unwrap(), original);
    assert_eq!(fs::read(malformed).unwrap(), b"invalid image fixture");
    let backup = parent.join("image-types.opvault");
    state.backup(&grant.token, &backup).unwrap();
    let restored = VaultState::new(
        parent.join("restored"),
        Box::new(|| Ok("one".into())),
        || true,
    );
    let restored_grant = restored.restore(&backup, PIN).unwrap();
    assert_eq!(
        restored_grant
            .items
            .iter()
            .map(|item| item.mime.as_str())
            .collect::<Vec<_>>(),
        vec!["image/png", "image/jpeg", "image/bmp"]
    );
    drop(restored);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}

#[test]
fn credential_budget_counts_utf8_bytes_and_unicode_scalars_consistently() {
    for value in ["é".repeat(128), "😀".repeat(64)] {
        assert!(validate_credential(&value).is_ok());
    }
    for value in ["é".repeat(129), "😀".repeat(65)] {
        assert!(validate_credential(&value)
            .unwrap_err()
            .contains("too long"));
    }
    assert!(validate_credential(&"😀".repeat(11)).is_err());
    assert!(validate_credential(&"😀".repeat(12)).is_ok());
}

#[test]
fn closed_backup_verification_refuses_tampering_and_cancellation_before_publication() {
    let (parent, state, _) = fixture();
    let grant = state.setup(PIN, PIN).unwrap();
    let source = parent.join("two-chunk.mp4");
    let original = vec![42; (CHUNK + 17) as usize];
    fs::write(&source, &original).unwrap();
    state.import(&grant.token, &source).unwrap();
    state.import(&grant.token, &photo(&parent)).unwrap();
    let backup = parent.join("valid.opvault");
    state.backup(&grant.token, &backup).unwrap();
    let saved = fs::read(&backup).unwrap();
    let lease = state.lease(&grant.token, false).unwrap();
    let (header, raw) = state.store().unwrap().state().unwrap();
    let catalog = catalog(&header, &lease.master).unwrap();
    transfer::verify_backup(&backup, &raw, &header.id, &catalog, &|| false).unwrap();
    let mut variants = Vec::new();
    for offset in [0, 8, 12, 12 + raw.len() + 8, saved.len() - 1] {
        let mut damaged = saved.clone();
        damaged[offset] ^= 1;
        variants.push(damaged);
    }
    variants.push(saved[..saved.len() - 1].to_vec());
    let mut appended = saved.clone();
    appended.push(0);
    variants.push(appended);
    for (index, damaged) in variants.iter().enumerate() {
        let destination = parent.join(format!("refused-{index}.opvault"));
        let mut staged = crate::safe_output::StagedOutput::new(&destination).unwrap();
        staged.file().write_all(damaged).unwrap();
        assert!(staged
            .verify_closed(|path| transfer::verify_backup(
                path,
                &raw,
                &header.id,
                &catalog,
                &|| false
            ))
            .is_err());
        assert!(staged.commit().is_err());
        assert!(!destination.exists());
    }
    let destination = parent.join("cancelled.opvault");
    let mut staged = crate::safe_output::StagedOutput::new(&destination).unwrap();
    staged.file().write_all(&saved).unwrap();
    let checks = std::sync::atomic::AtomicUsize::new(0);
    assert!(staged
        .verify_closed(
            |path| transfer::verify_backup(path, &raw, &header.id, &catalog, &|| checks
                .fetch_add(1, Ordering::SeqCst)
                >= 3)
        )
        .is_err());
    assert!(staged.commit().is_err());
    assert!(!destination.exists());
    let object_path = state
        .store()
        .unwrap()
        .object_path(&catalog.items[0].id)
        .unwrap();
    let stored = fs::read(&object_path).unwrap();
    let mut damaged = stored.clone();
    let last = damaged.len() - 1;
    damaged[last] ^= 1;
    fs::write(&object_path, &damaged).unwrap();
    let refused = parent.join("bad-source.opvault");
    assert!(state.backup(&grant.token, &refused).is_err());
    assert!(!refused.exists());
    fs::write(&object_path, stored).unwrap();
    assert_eq!(fs::read(&backup).unwrap(), saved);
    assert_eq!(fs::read(source).unwrap(), original);
    drop(state);
    fs::remove_dir_all(parent).unwrap();
}
