# Windows installed XMP safety acceptance runbook (F07/R01)

**Status:** Manual acceptance procedure, **not** evidence of a completed test. Keep [Issue #90](https://github.com/MieleMadness/Open-Photo/issues/90) open until all requirements and the exact source commit are verified. The automated full-installer job does not perform external-editor concurrency trials.

## 1. Environment and source integrity

Record the installed Open-Photo version, commit, workflow run/artifact, Windows version/build, filesystem (NTFS/ReFS/network), running account, and external editor/version. Test in a dedicated disposable registered folder. **Never use personal originals as test fixtures.** Include 2 synthetic JPEGs (and optionally a disposable RAW/video fixture), a managed Open-Photo XMP sidecar, and a third-party XMP sidecar with unknown namespaced fields.

Before every trial record SHA-256 hashes for original media and existing sidecar(s) with `Get-FileHash -Algorithm SHA256`. After each trial recalculate source hashes and compare. No trial passes if any original-media byte changes.

## 2. Installed baseline smoke test

- [ ] Install the exact CI artifact; record its SHA-256 and version.
- [ ] Open a disposable registered media folder, create a first managed keyword sidecar, and verify that the original media hash is unchanged.
- [ ] Reopen the same photo, add another keyword, and verify expected tag merge, third-party property preservation, and idempotence after repeating the batch.
- [ ] Place a foreign-owned XMP sidecar beside another synthetic image. Verify that tagging refuses to overwrite or silently adopt that file.
- [ ] Place malformed XMP beside a third image and verify the malformed bytes survive unmodified and other batch entries still complete.

## 3. Stale reads and real external-editor conflicts

Perform a trial with ExifTool or another independent XMP editor that can modify the disposable sidecar. Save the external sidecar *after* Open-Photo previews/reads its old state and *before* triggering Open-Photo's save. Confirm that Open-Photo refuses a stale update, leaves the external editor's bytes intact, and provides an actionable error.

**Unresolved race gate:** Use a deterministic test harness or filesystem hook to update or swap the sidecar **between Open-Photo's final content check and `ReplaceFileW`**. A successful overwrite of externally changed bytes is a **FAIL and release blocker**, even if a preflight stale-read trial passes. Do not substitute rapid UI clicking for deterministic boundary injection. Record external-process logs, old/new hashes, and evidence of the exact interleaving.

- [ ] External edit before commit: no silent overwrite
- [ ] Equal-byte-length external edit: conflicts detected by bytes, not timestamps
- [ ] External rename/replacement during final compare-to-publish window: no lost external changes
- [ ] Two simultaneous independent writers: no last-writer-wins silent data loss
- [ ] No commit error may report a successful save if it left a backup or failed to publish

## 4. Recovery artifacts and interrupted writes

Only in the disposable directory, create `photo.jpg.xmp.openphoto.tmp` and `photo.jpg.xmp.openphoto.bak` with different known byte sequences. Also test a dangling symlink/junction where Windows permissions support creation. For each test, record the sidecar hash and all recovery artifacts before and after the operation.

- [ ] Existing temp artifact: operation refuses, sidecar/temp bytes unchanged
- [ ] Existing backup artifact: operation refuses, sidecar/backup bytes unchanged
- [ ] Dangling temporary/backup link: operation refuses without following the link
- [ ] Denied backup write / low disk space: no success claim or original-sidecar mutation
- [ ] Injected publication failure: previous sidecar intact and recoverable backup preserved
- [ ] Abrupt exit after staging, backup, publish, and before acknowledgement: restart twice; show consistent, reviewable recovery status without automatic destructive cleanup

**Do not test crash injection by force-terminating against non-disposable user data.** No procedure here authorizes automatic selection or deletion of ambiguous recovery files.

## 5. Windows path/handle identity

- [ ] Registered root removed/revoked during operation: abort without writes outside authorization
- [ ] Junction or reparse-point alias outside registered root: refuse
- [ ] Hard-link alias and symlink changes: refuse or demonstrate safe handle-bound identity
- [ ] Read-only destination / sharing violation: fail closed and preserve recovery material
- [ ] Verify actual `ReplaceFileW` behavior against an independent writer on the installed target Windows/filesystem, not inferred POSIX rename semantics

## 6. Signoff and evidence

| Date | Installed commit/artifact | Case | Before/after SHA-256 | Observed error / recovery state | Pass/Fail | Issue/evidence |
| --- | --- | --- | --- | --- | --- | --- |
| | | | | | | |

**Gate:** Exact-head frontend, native Rust and Windows packaging CI are necessary but not sufficient. F07/R01 remains open until the real compare-to-publish race, crash durability/recovery, Windows root/reparse identity, and user-reviewable conflict handling have passing evidence. Preserve original source media read-only. See [XMP fault matrix](AUDIT_XMP_RECOVERY_GATE.md) and [Issue #90](https://github.com/MieleMadness/Open-Photo/issues/90).
