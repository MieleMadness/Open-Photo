//! Disposable NTFS sharing-mode characterization for Issue #90.
//! These tests DO NOT implement the external-editor publication lock.
#![cfg(windows)]

use std::{
    fs,
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_READ};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    destination: PathBuf,
    staged: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = loop {
            let dir = std::env::temp_dir().join(format!(
                "openphoto-windows-share-experiment-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&dir) {
                Ok(()) => break dir,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("Unable to create synthetic fixture: {error}"),
            }
        };
        let destination = root.join("photo.jpg.xmp");
        let staged = root.join("photo.jpg.xmp.openphoto.tmp");
        fs::write(&destination, b"original-sidecar-bytes").unwrap();
        fs::write(&staged, b"staged-sidecar-bytes").unwrap();
        Self {
            root,
            destination,
            staged,
        }
    }

    fn read_guard(&self) -> fs::File {
        fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
            .open(&self.destination)
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn open_read_guard_denies_new_writable_handle_until_released() {
    let fixture = Fixture::new();
    let guard = fixture.read_guard();
    let conflict = fs::OpenOptions::new()
        .write(true)
        .open(&fixture.destination)
        .unwrap_err();
    assert_eq!(
        conflict.raw_os_error(),
        Some(32),
        "Expected Windows ERROR_SHARING_VIOLATION for a competing writer"
    );
    assert_eq!(
        fs::read(&fixture.destination).unwrap(),
        b"original-sidecar-bytes"
    );

    drop(guard);
    let writable = fs::OpenOptions::new()
        .write(true)
        .open(&fixture.destination)
        .unwrap();
    drop(writable);
    assert_eq!(
        fs::read(&fixture.destination).unwrap(),
        b"original-sidecar-bytes"
    );
}

#[test]
fn replacement_with_read_guard_is_characterized_without_assuming_cas() {
    let fixture = Fixture::new();
    let guard = fixture.read_guard();

    // Whether ReplaceFileW succeeds while this handle is open is recorded
    // but cannot alone establish a compare-and-swap protocol or protect
    // against previously opened external writable handles.
    let result = crate::atomic_replace(&fixture.staged, &fixture.destination);
    let bytes = fs::read(&fixture.destination).unwrap();
    if result.is_ok() {
        assert_eq!(bytes, b"staged-sidecar-bytes");
        assert!(
            !fixture.staged.exists(),
            "Successful replacement must consume the staged file"
        );
    }
    assert!(
        bytes == b"original-sidecar-bytes" || bytes == b"staged-sidecar-bytes",
        "No partially replaced destination is acceptable: {bytes:?}"
    );
    eprintln!("Windows ReplaceFileW with held read handle: {result:?}");
    drop(guard);
}

// The parent acquires its guard before launching this helper, then retains it
// until the child exits. No sleeps or probabilistic scheduling are involved.
#[test]
fn external_replacement_child() {
    let Some(root) = std::env::var_os("OPENPHOTO_SHARING_CHILD_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let staged = root.join("external.tmp");
    let destination = root.join("photo.jpg.xmp");
    fs::write(&staged, b"external-editor-bytes").unwrap();
    let result = crate::atomic_replace(&staged, &destination);
    if std::env::var_os("OPENPHOTO_SHARING_DENY_DELETE").is_some() {
        let error = result.expect_err("Delete-denying guard must refuse external replacement");
        assert_eq!(error.raw_os_error(), Some(32));
        assert_eq!(fs::read(&destination).unwrap(), b"original-sidecar-bytes");
        assert_eq!(fs::read(&staged).unwrap(), b"external-editor-bytes");
    } else {
        result.expect("Delete-sharing guard permits external pathname replacement");
        assert_eq!(fs::read(&destination).unwrap(), b"external-editor-bytes");
        assert!(!staged.exists());
    }
}

fn external_process_replacement(deny_delete: bool) {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Seek, SeekFrom};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let fixture = Fixture::new();
    let source = fixture.root.join("photo.jpg");
    fs::write(&source, b"disposable-source-media").unwrap();
    let source_before = fs::read(&source).unwrap();
    let source_hash = Sha256::digest(&source_before);
    let mut guard = fs::OpenOptions::new()
        .read(true)
        .share_mode(if deny_delete {
            FILE_SHARE_READ
        } else {
            FILE_SHARE_READ | FILE_SHARE_DELETE
        })
        .open(&fixture.destination)
        .unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "xmp_windows_sharing_tests::external_replacement_child",
            "--nocapture",
        ])
        .env("OPENPHOTO_SHARING_CHILD_ROOT", &fixture.root)
        .env_remove("OPENPHOTO_SHARING_DENY_DELETE")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if deny_delete {
        command.env("OPENPHOTO_SHARING_DENY_DELETE", "1");
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let output = child.wait_with_output().unwrap();
            panic!("External replacement helper timed out: {output:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "External replacement helper failed: {output:?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
        "The exact child test must execute, not merely exit successfully: {output:?}"
    );
    guard.seek(SeekFrom::Start(0)).unwrap();
    let mut retained_bytes = Vec::new();
    guard.read_to_end(&mut retained_bytes).unwrap();
    assert_eq!(retained_bytes, b"original-sidecar-bytes");
    let path_bytes = fs::read(&fixture.destination).unwrap();
    assert_eq!(
        path_bytes,
        if deny_delete {
            b"original-sidecar-bytes".as_slice()
        } else {
            b"external-editor-bytes".as_slice()
        }
    );
    if deny_delete {
        // The same policy blocks our own publication while the guard is held.
        let error = crate::atomic_replace(&fixture.staged, &fixture.destination).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(32));
        assert_eq!(fs::read(&fixture.staged).unwrap(), b"staged-sidecar-bytes");
        assert_eq!(
            fs::read(&fixture.destination).unwrap(),
            b"original-sidecar-bytes"
        );
    }
    assert_eq!(fs::read(&source).unwrap(), source_before);
    assert_eq!(Sha256::digest(fs::read(&source).unwrap()), source_hash);
    eprintln!("External-process replacement deny_delete={deny_delete}: retained={retained_bytes:?}, pathname={path_bytes:?}");
    drop(guard);
}

#[test]
fn external_process_replacement_detaches_delete_sharing_guard_from_pathname() {
    external_process_replacement(false);
}

#[test]
fn denying_delete_sharing_blocks_external_and_own_publication() {
    external_process_replacement(true);
}

// Keep subprocess lifetime bounded even if a parent assertion panics. In the
// writer-first experiment the child deliberately waits for an explicit release.
struct TestChild(std::process::Child);

impl TestChild {
    fn wait(mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success(), "Child failed: {status}");
                return;
            }
            assert!(std::time::Instant::now() < deadline, "Child timed out");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

impl Drop for TestChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[test]
fn writer_first_child() {
    use std::io::{Read, Write};
    let Some(root) = std::env::var_os("OPENPHOTO_WRITER_FIRST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .open(root.join("photo.jpg.xmp"))
        .unwrap();
    // Signal only after acquisition: the parent cannot race this open.
    fs::write(root.join("writer-ready"), b"ready").unwrap();
    let mut release = [0];
    std::io::stdin().read_exact(&mut release).unwrap();
    assert_eq!(release, [1]);
    writer.set_len(0).unwrap();
    writer.write_all(b"external-editor-bytes").unwrap();
    writer.sync_all().unwrap();
    drop(writer);
    fs::write(root.join("writer-finished"), b"finished").unwrap();
}

#[test]
fn preexisting_external_process_writer_refuses_guard_then_allows_fresh_retry() {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let fixture = Fixture::new();
    let source = fixture.root.join("photo.jpg");
    fs::write(&source, b"disposable-source-media").unwrap();
    let source_hash = Sha256::digest(fs::read(&source).unwrap());
    let mut child = TestChild(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "xmp_windows_sharing_tests::writer_first_child",
                "--nocapture",
            ])
            .env("OPENPHOTO_WRITER_FIRST_ROOT", &fixture.root)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !fixture.root.join("writer-ready").exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "Writer exited before acquiring handle"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "Writer readiness timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let error = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(&fixture.destination)
        .unwrap_err();
    assert_eq!(error.raw_os_error(), Some(32));
    assert_eq!(
        fs::read(&fixture.destination).unwrap(),
        b"original-sidecar-bytes"
    );
    child.0.stdin.take().unwrap().write_all(&[1]).unwrap();
    child.wait();
    // This marker also proves the exact child test actually ran to completion.
    assert_eq!(
        fs::read(fixture.root.join("writer-finished")).unwrap(),
        b"finished"
    );
    let mut fresh_guard = fixture.read_guard();
    let mut fresh_bytes = Vec::new();
    fresh_guard.read_to_end(&mut fresh_bytes).unwrap();
    assert_eq!(fresh_bytes, b"external-editor-bytes");
    assert_ne!(fresh_bytes, b"original-sidecar-bytes");
    assert_eq!(fs::read(&fixture.staged).unwrap(), b"staged-sidecar-bytes");
    assert_eq!(Sha256::digest(fs::read(&source).unwrap()), source_hash);
}

fn handle_identity(file: &fs::File) -> (u32, u64) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: the owned file handle is live and success initializes the buffer.
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) };
    assert_ne!(
        ok,
        0,
        "Cannot inspect fixture identity: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: the API succeeded above.
    let info = unsafe { info.assume_init() };
    let identity = (
        info.dwVolumeSerialNumber,
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    );
    assert_ne!(identity.0, 0, "Fixture volume identity unavailable");
    assert_ne!(identity.1, 0, "Fixture file identity unavailable");
    identity
}

#[test]
fn hardlink_alias_keeps_old_identity_after_identical_content_replacement() {
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new();
    let source = fixture.root.join("photo.jpg");
    fs::write(&source, b"disposable-source-media").unwrap();
    let source_hash = Sha256::digest(fs::read(&source).unwrap());
    let alias = fixture.root.join("alias.xmp");
    fs::hard_link(&fixture.destination, &alias).unwrap();
    let guard = fixture.read_guard();
    let alias_handle = fs::File::open(&alias).unwrap();
    let old_identity = handle_identity(&guard);
    assert_eq!(handle_identity(&alias_handle), old_identity);
    // Identical bytes at a different identity defeat content-only checks.
    fs::write(&fixture.staged, b"original-sidecar-bytes").unwrap();
    let staged_handle = fs::File::open(&fixture.staged).unwrap();
    let staged_identity = handle_identity(&staged_handle);
    assert_ne!(staged_identity, old_identity);
    drop(staged_handle);
    crate::atomic_replace(&fixture.staged, &fixture.destination).unwrap();
    assert!(!fixture.staged.exists());
    let current = fs::File::open(&fixture.destination).unwrap();
    assert_eq!(handle_identity(&current), staged_identity);
    assert_eq!(handle_identity(&guard), old_identity);
    assert_eq!(handle_identity(&alias_handle), old_identity);
    assert_eq!(
        fs::read(&fixture.destination).unwrap(),
        fs::read(&alias).unwrap()
    );
    assert_eq!(Sha256::digest(fs::read(&source).unwrap()), source_hash);
}

#[test]
fn parent_rename_is_refused_with_live_child_then_rebinding_changes_identity() {
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new();
    let source = fixture.root.join("photo.jpg");
    fs::write(&source, b"disposable-source-media").unwrap();
    let source_hash = Sha256::digest(fs::read(&source).unwrap());
    let workspace = fixture.root.join("workspace");
    let moved_workspace = fixture.root.join("moved-workspace");
    fs::create_dir(&workspace).unwrap();
    let destination = workspace.join("photo.jpg.xmp");
    fs::write(&destination, b"original-sidecar-bytes").unwrap();
    let guard = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(&destination)
        .unwrap();
    let original_identity = handle_identity(&guard);
    let error = fs::rename(&workspace, &moved_workspace).unwrap_err();
    assert!(
        matches!(error.raw_os_error(), Some(5 | 32)),
        "Unexpected directory rename failure: {error}"
    );
    assert!(!moved_workspace.exists());
    assert_eq!(fs::read(&destination).unwrap(), b"original-sidecar-bytes");
    assert_eq!(handle_identity(&guard), original_identity);
    // Deliberately model the unprotected interval after release. Production
    // must not use this unlock-and-retry sequence as conflict prevention.
    drop(guard);
    fs::rename(&workspace, &moved_workspace).unwrap();
    fs::create_dir(&workspace).unwrap();
    fs::write(&destination, b"original-sidecar-bytes").unwrap();
    let rebound = fs::File::open(&destination).unwrap();
    let moved = fs::File::open(moved_workspace.join("photo.jpg.xmp")).unwrap();
    assert_eq!(handle_identity(&moved), original_identity);
    assert_ne!(handle_identity(&rebound), original_identity);
    assert_eq!(
        fs::read(&destination).unwrap(),
        fs::read(moved_workspace.join("photo.jpg.xmp")).unwrap()
    );
    assert_eq!(Sha256::digest(fs::read(&source).unwrap()), source_hash);
}
