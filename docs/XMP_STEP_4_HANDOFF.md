# XMP step 4 — local installer test handoff

Candidate version: **0.56.70**, unpublished. The user requested preparation of
the installer and handoff locally, without installation on this computer.
Step 4 installed acceptance is **pending**, not complete.

## Package and provenance

The delivery folder is `audit-output/step4-handoff` beside the source checkout.
It contains the installer, SHA-256 manifest, a source snapshot (including local
uncommitted changes), build evidence and this checklist. `handoff.json` records
the observed build/package results and remaining gates. The Git base revision
alone does not identify the candidate: use `source-files.json` and the snapshot.
Documentation may be finalized after compilation; application source is not
changed between compilation and snapshot preparation.

Run `verify-handoff.ps1` in that folder before testing. It verifies the delivered
files and source snapshot against recorded hashes; it never launches the app or
installer. This detects changes relative to this handoff, not publisher trust.
The candidate is not a signed or published release. No updater artifacts are
prepared. Source dependencies, model downloads and Windows build prerequisites
are not included in the source snapshot.

The fresh frontend and optimized native builds passed. Staged-input checks
verified seven AI assets, eleven native runtime files and nine video-engine
files against their manifests; these are not installed-payload checks. Initial
NSIS tool extraction failed with access denied; a bundle-only retry passed tool
setup. That retry warned that the already-patched executable no longer contained
the bundle-type placeholder. Both logs are retained; updater behavior has not
been accepted. The existing frontend bundle-size warning also remains.

## Disposable Windows GUI test

Use a Windows x64 VM or dedicated test machine with a fresh disposable Windows
profile, WebView2 and a snapshot to return to. Copy only expendable media fixtures
into it; do not attach a personal library. Record Windows/WebView2 versions,
filesystem, display scale, installer hash and test results. Preserve screenshots,
sidecars and journal/backup files on failure before reverting the VM.

1. Install the candidate and launch it. Confirm version 0.56.70. Exercise both
   light/dark themes, keyboard confirmation/cancellation and a small window.
2. Add copied JPEG/TIFF/HEIC and MOV/MP4 fixtures as a workspace. Record originals'
   SHA-256 hashes before testing. Confirm thumbnails, metadata and normal tags.
3. Add a tag to media without a sidecar. Confirm a new XMP sidecar and tag readback.
   Put a foreign sidecar beside another fixture; confirm it remains untouched.
4. On an Open-Photo-managed sidecar, propose a second tag. In **Tags → XMP changes
   to review**, compare original/proposal/current, cancel once, then obtain a
   fresh comparison and confirm **Apply reviewed changes**. Check the result,
   completed history, retained original backup and tag refresh.
5. Change the sidecar using ExifTool after comparison and before confirmation.
   Confirm stale approval refuses the save and preserves the editor's version.
   Review again. Also test an external edit after a successful save; reopening
   history must not replace it. Record the editor version and command used.
6. Rename/move a source or remove its workspace folder after proposal. Confirm
   Apply refuses it, and journal-only cancellation remains possible. Switch
   workspaces during a review and check that late results do not affect the new
   workspace. Test a read-only fixture folder: refusal must preserve all files.
7. Recovery needs a deliberately interrupted save fixture on the disposable
   system. Do not attempt to time termination against personal data. Use the
   native step-3 process-exit fixtures for deterministic boundaries; ordinary
   GUI termination may miss the interval. Reopen twice and confirm no automatic
   winner/cleanup. For a missing destination and verified original, freshly
   compare and explicitly confirm restoration. A competing file, damaged backup,
   replacement identity or unsafe link must block restoration and preserve both.
   If no interrupted GUI case is reproduced, record this case as not run.
8. Smoke-test local AI, video preview and export to a new filename; confirm
   no-overwrite behavior. Compare originals' SHA-256 hashes with the baseline.
   Reopen the app and verify history and resulting tags persist.

Automated race/process-exit tests from step 3 are complementary evidence. They
do not establish installed WebView2 acceptance, hardware power-loss durability,
network filesystem safety or all filesystem filter/reparse behavior. The save
protocol can briefly leave a destination name absent. Source identity checks
do not persist a hash of every original byte between proposal and Apply.

## Installer lifecycle and native acceptance

`scripts/test-installed-app.ps1` is deliberately restricted to disposable
**GitHub-hosted Windows runners** because it writes/removes application data.
Never spoof its environment checks to run it in a VM or personal profile.
When remote validation is authorized later, use the existing full Windows
workflow (`full_windows=true`) against a committed snapshot matching this code.
That workflow rebuilds its own installer; compare the source snapshot rather
than claiming it tests this local binary. No workflow was dispatched here.

The guarded script verifies installed AI files/inference, runtime dependencies,
update/default uninstall retention and explicit managed-data purge with an
unknown-media sentinel preserved. Retain its JSON reports. In a disposable VM,
manual install/uninstall checks can be recorded separately; they do not count
as a passing run of that script.

For native checks from the snapshot, install the documented Node/Rust/MSVC build
prerequisites, run `npm ci`, `npm run prepare:video`, `npm run prepare:models`,
then build/test following `.github/workflows/windows.yml`. The source snapshot
does not include the official ExifTool checkout used locally in step 3; obtain
the pinned version/commit recorded in the step-3 evidence if repeating that
explicit matrix. Run `scripts/accept-video.ps1` into a fresh output directory
for release video acceptance. Record any modes not run rather than counting
ignored native tests as passed acceptance.

## Release gates still open

- Installed WebView2 XMP flow and real supported-media QA.
- Installed AI/runtime and install/update/uninstall acceptance.
- Release video export acceptance and remaining explicit acceptance modes.
- Storage/permission coverage and hardware power-loss durability.
- Exact-source remote CI before merge/release; Issue #90 remains open.

Keep all output local until separately authorized. See
[step 3 validation](XMP_STEP_3_VALIDATION.md) for the completed local tests.
