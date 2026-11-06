//! Characterize opening a new no-write-sharing guard after an external
//! writer already has a writable handle. Synthetic, Windows-only evidence.
#![cfg(windows)]

use std::{
    fs,
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    sidecar: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = loop {
            let candidate = std::env::temp_dir().join(format!(
                "openphoto-preexisting-writer-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("Cannot create disposable fixture: {error}"),
            }
        };
        let sidecar = root.join("synthetic-photo.jpg.xmp");
        fs::write(&sidecar, b"external-base").unwrap();
        Self { root, sidecar }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn preexisting_writable_handle_blocks_later_no_write_share_guard() {
    let fixture = Fixture::new();
    let external = fs::OpenOptions::new()
        .write(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(&fixture.sidecar)
        .unwrap();

    let result = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(&fixture.sidecar);
    let error = result.expect_err("Pre-existing writer must prevent acquiring a no-write-share guard");
    assert_eq!(error.raw_os_error(), Some(32));

    assert_eq!(fs::read(&fixture.sidecar).unwrap(), b"external-base");
    drop(external);

    let guard = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
        .open(&fixture.sidecar)
        .unwrap();
    assert_eq!(fs::read(&fixture.sidecar).unwrap(), b"external-base");
    drop(guard);
}
