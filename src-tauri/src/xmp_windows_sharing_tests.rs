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
    assert!(
        bytes == b"original-sidecar-bytes" || bytes == b"staged-sidecar-bytes",
        "No partially replaced destination is acceptable: {bytes:?}"
    );
    eprintln!(
        "Windows ReplaceFileW with held read handle: {}",
        if result.is_ok() {
            "succeeded"
        } else {
            "failed"
        }
    );
    drop(guard);
}
