# Windows XMP file-sharing experiment (Issue #90)

**Status:** Test-only characterization, not a production conflict-prevention implementation, installed-app acceptance, or release permission.

Two Windows-only native tests use disposable synthetic sidecars. They exercise an open destination read handle with `FILE_SHARE_READ | FILE_SHARE_DELETE` (excluding `FILE_SHARE_WRITE`):

1. Verify a **new** writable handle is refused with Windows `ERROR_SHARING_VIOLATION` while the read handle is retained, and succeeds after it closes.
2. Call the existing `ReplaceFileW` wrapper while that read handle is retained. A successful result must publish the complete staged bytes and consume the staged file. On error, require complete old or new destination bytes. Record the full result in CI; neither outcome alone proves conflict prevention.

This **does not** prove safety against an independent editor that already held a writable handle before the guard, a different filesystem/share implementation, external rename/reparse changes, source identity mismatch, power loss, or a time-of-check/time-of-use race. An eventual production protocol needs handle-bound file identity, an authoritative final content comparison, installed NTFS sharing/rename trials, explicit failure/backup behavior, and a passing (currently ignored) successful-publication race gate. Keep [Issue #90](https://github.com/MieleMadness/Open-Photo/issues/90) open.

Run the Windows-only native checks on the exact commit:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --locked --lib xmp_windows_sharing_tests
```

The regular Windows Rust CI job runs these tests; the full Windows installer is not required for this synthetic characterization. Do not use personal media to reproduce them.

## Independent-process publication experiments

The next synthetic tests acquire the parent read guard **before** starting a
separate instance of the native test executable. The parent retains its guard
until child exit, checks the exit status and bounds the child to twenty seconds.
The helper uses the production `atomic_replace` wrapper on disposable files.

- With delete sharing allowed, require successful external replacement, consumed
  staging, original bytes through the retained handle and external bytes through
  the pathname. This demonstrates why a retained handle alone is insufficient.
- With delete sharing denied, require sharing violation 32 for both external
  replacement and Open-Photo's own publication. Both staged candidates and the
  destination must remain intact. There is no unlock-before-replace fallback.
- Compare disposable source-media bytes before/after each experiment. Neither
  experiment changes production save behavior or establishes file identity,
  crash durability, network/removable filesystem support or installed acceptance.

The `external_replacement_child` helper returns immediately in ordinary runs;
its assertions execute only when launched with the parent's fixture environment.
Keep Issue #90 open. The continuation below records handle-bound file identity,
parent-directory rename and writer-first process coverage. Reparse/root checks
and the production protocol remain pending.

### Local Windows result, November 5, 2026

Both independent-process experiments passed: delete sharing allowed replacement
while the retained handle continued reading original bytes; denying delete
sharing returned error 32 for both publishers. All five sharing-suite tests
passed. Source byte and SHA-256 checks and child-test execution checks passed.
This is local synthetic evidence, not installed-app or crash-durability proof.

## Local continuation: writer-first and identity checks

Three further experiments extend the local suite:

- A separate writer process signals readiness only after acquiring a writable
  handle. The parent must receive sharing violation 32 when acquiring its read
  guard. An explicit stdin release lets the writer publish and close; a fresh
  guard must then read the external bytes, and staging remains unconsumed.
  The completion marker proves helper execution. Child cleanup kills/reaps a
  waiting process on parent failure, with bounded readiness and completion waits.
- Hardlink aliases initially share the same handle-bound volume/file ID. After
  replacing the destination with an identical-content staged file, the current
  path has the staged identity while the retained guard and alias keep the old
  identity. The staging read handle is closed before ReplaceFileW: retaining it
  was observed to cause sharing violation 32 on this local Windows machine.
- Renaming a parent directory with a live child-file read guard is refused
  (access denied 5 locally; sharing violation 32 is also accepted). Verify the
  original path and bytes survive. After explicitly releasing that handle,
  rename and recreate the old directory path with identical sidecar contents:
  the old pathname now identifies a different file. This deliberately models
  an unprotected interval, not a safe production retry strategy.

Each experiment checks SHA-256 preservation of disposable source media. These
are local Windows filesystem experiments, not junction/reparse validation,
registered-root authorization, crash durability or a production protocol.

### Local verification checkpoint

All nine Windows sharing tests passed. The complete native suite passed with
281 tests passed and eight explicit acceptance tests ignored. Warnings-as-errors
Clippy (`--lib --tests`) and formatting passed. Logs are retained locally in
`../audit-output/xmp-identity-local.log`, `xmp-identity-native.log` and
`xmp-identity-clippy.log`. This update changes tests/documentation only, so the
previously verified frontend build was not repeated. No GitHub publication,
remote CI, installer or installed-app acceptance was performed for this update.

Next local work: junction/reparse and registered-root identity checks; then a
reviewable production publication design with explicit refusal/candidate
preservation when identity cannot remain protected. Crash-state persistence and
user recovery decisions remain separate required work under Issue #90.
