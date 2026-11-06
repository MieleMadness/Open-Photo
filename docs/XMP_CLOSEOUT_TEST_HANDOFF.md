# XMP 0.56.76 — local test handoff

This is the final local XMP engineering candidate, prepared for later acceptance.
Nothing has been installed on the personal profile or uploaded. Use this
candidate instead of the preserved 0.56.70 handoff for future testing.

## Verify the delivery

Copy the complete `xmp-closeout-0.56.76` directory. Run `verify-handoff.ps1` to
check the installer, source snapshot, evidence and pre-build source hashes. It
does not launch or install the app. `handoff.json` lists actual results and
unrun gates; `SHA256.json` identifies delivered files. Hashes are not a publisher
signature. The source has local uncommitted changes: Git HEAD alone is not its
identity. Bundled runtime DLLs and AI/video resources are in the installer, not
the source ZIP; source rebuilds need the pinned preparation scripts and normal
build tools.

The `evidence/xmp-closeout-intent-before.log` failure is the deliberate pre-fix
reproduction. `intent-after`, the final native log and the candidate source show
its correction. Do not count the historical failure as an outstanding test or
silently discard it. Build-input/resource checks describe compiler inputs, not
an extracted or installed payload. No updater acceptance is claimed.
The superseded `installer-before-format` log is an interrupted precursor;
`xmp-closeout-installer.log` records the final formatted-source package build.

## Later disposable-Windows checks

Use a fresh Windows x64 VM/test profile with WebView2 and a return-to snapshot.
Only attach copied disposable fixtures. Record Windows, WebView2, display scale,
filesystem, installer SHA-256 and before/after original-media hashes.

1. Install and confirm **0.56.76**. Open copied real JPEG/TIFF/HEIC and MOV/MP4
   media, inspect previews/metadata and switch light/dark at normal/small sizes.
2. Create a new sidecar. Try a foreign sidecar and confirm it stays untouched.
   Propose a managed merge, inspect all versions and confirm Apply. Check tags,
   preserved unrelated metadata, recovery original and completed history.
3. Make an external-editor change between review and confirmation. Stale Apply
   must refuse and preserve that version. Repeat source-content changes; fresh
   approval cannot rebind an old proposal. Cancel/repropose deliberately.
4. Exercise source/folder unavailability and removed-root authorization. Stored
   versions remain readable; unverified writes/restoration remain blocked.
   Ordinary pending cancellation is journal-only. Switch workspace while loading
   and confirm late results do not alter the new workspace.
5. Exercise explicit recovery using the native process-exit fixtures and an
   installed GUI interruption if one can be safely reproduced. Reopen twice:
   no automatic winner/cleanup. Restore only a verified original into an empty
   path after fresh confirmation. A competing file or damaged/replaced backup
   must win/refuse restoration. Record an unreproduced GUI interruption as not run.
6. Inspect staging observations: links/folders are unverified and not followed;
   partial scans are not called clear. There are no stage promotion/deletion
   controls. Source originals and every ambiguous recovery artifact stay intact.
7. Browse older pending/history pages, refresh, retry a failed check and verify
   old confirmation is cleared. Check keyboard focus and no horizontal overflow.
8. Smoke-test local AI, video preview and new-file export. Verify no-overwrite
   exports and original-media hashes. Reopen to check durable results/history.

The included synthetic PNG/XMP fixture ZIP supports repeatable basic cases; it
does not replace real-media, installed, sudden-power-loss or storage-failure QA.
The two-phase sidecar save can briefly leave its destination name absent.

## Lifecycle and release gates

The existing `scripts/test-installed-app.ps1` intentionally runs only on disposable
**GitHub-hosted Windows runners** because it writes/deletes app data. Do not spoof
the guard to run it here or in a personal VM profile. Manual disposable-VM
install/uninstall checks can be recorded separately. When remote work is later
authorized, run the existing full Windows workflow against a committed source
matching this snapshot; it rebuilds a candidate rather than proving this local
binary installed correctly. Keep its installed AI/runtime, upgrade/uninstall and
release-video reports separate from browser/native results.

Installed acceptance, remaining explicit modes, storage/permission coverage,
hardware power-loss durability and exact-source remote CI still gate release.
This closeout does not close Issue #90. See [local closeout](XMP_CLOSEOUT.md) for implementation
evidence and the next coding-track boundary.
