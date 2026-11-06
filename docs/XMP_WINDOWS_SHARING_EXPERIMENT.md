# Windows XMP file-sharing experiment (Issue #90)

**Status:** Test-only characterization, not a production conflict-prevention implementation, installed-app acceptance, or release permission.

Two Windows-only native tests use disposable synthetic sidecars. They exercise an open destination read handle with `FILE_SHARE_READ | FILE_SHARE_DELETE` (excluding `FILE_SHARE_WRITE`):

1. Verify a **new** writable handle is refused with Windows `ERROR_SHARING_VIOLATION` while the read handle is retained, and succeeds after it closes.
2. Call the existing `ReplaceFileW` wrapper while that read handle is retained. Record whether it succeeds or fails and assert the destination is either complete old bytes or complete new bytes—never partial fixture data.

This **does not** prove safety against an independent editor that already held a writable handle before the guard, a different filesystem/share implementation, external rename/reparse changes, source identity mismatch, power loss, or a time-of-check/time-of-use race. An eventual production protocol needs handle-bound file identity, an authoritative final content comparison, installed NTFS sharing/rename trials, explicit failure/backup behavior, and a passing (currently ignored) successful-publication race gate. Keep [Issue #90](https://github.com/MieleMadness/Open-Photo/issues/90) open.

Run the Windows-only native checks on the exact commit:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib xmp_windows_sharing_tests
```

The regular Windows Rust CI job runs these tests; the full Windows installer is not required for this synthetic characterization. Do not use personal media to reproduce them.
