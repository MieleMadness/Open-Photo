# Reviewed XMP Apply — local step 2 checkpoint

Implemented November 6, 2026. Local working-tree update only: no commit, push,
remote CI, installer or installed-app update. This supersedes the earlier
proposal-only behavior. It is not release or power-loss acceptance.

## User workflow

Adding tags still creates new sidecars without replacing an existing file.
Changes to managed sidecars become proposals under **Tags → XMP changes to
review**. Inspect the current, proposed and original versions, select
**I reviewed the current and proposed versions**, then **Apply reviewed changes**.

Approval is bound to the workspace, operation, candidate hash, original sidecar
identity/hash, source media identity, registered-folder identity and current
sidecar identity/content. The backend revalidates those bindings under retained
handles; the checkbox alone grants no filesystem access. A proposal changed
between listing and inspection is rejected by the UI and the Apply command.
Replaced media, changed sidecars, altered proposals, removed registration,
changed root identity and unresolved legacy recovery entries refuse Apply.
Older proposals without source/folder identity fields must be cancelled and
recreated; schema migration does not rewrite any media or sidecars.

## Save protocol and limits

1. Acquire Windows handles protecting the media, sidecar and ancestor directories.
   Existing writable or delete-capable sidecar handles prevent acquiring the
   read/delete guard with write/delete sharing denied. Original media is opened
   for reading only. Storage remains limited to fixed local NTFS.
2. Persist an `applying` intent, exact original/candidate snapshots and hashes,
   identities and a unique recovery-original filename in FULL-sync SQLite.
3. Recheck saved-workspace authorization in an immediate transaction. Keep the
   filesystem guards retained across this boundary.
4. Rename the original sidecar to its recovery filename using its already-owned
   read/delete handle and the retained parent handle, without replacement.
   Other processes cannot write/delete that protected file while it is retained.
5. Flush an exclusively created candidate stage, reopen and verify its identity
   and bytes with write sharing denied, then publish through native
   NtSetInformationFile(FileRenameInformation) with ReplaceIfExists=false.
   The parent is a retained handle; the final operation never resolves its
   directory by a fresh pathname lookup.
6. Record `applied` only after publication returns successfully. Keep the exact
   old sidecar as a physical recovery original and retain both snapshots in SQLite.

**This is not a single atomic replacement transaction.** There is an interval
between moving the original and publishing the candidate when the destination
name is absent. Readers may observe absence. An editor creating a destination
during that interval wins: Open-Photo refuses to overwrite it and retains the
old sidecar and proposed version. Complete candidate publication is no-replace;
continuous presence of the destination name is not promised.

On a handled error, restoration uses only the retained original handle and a
no-overwrite rename. If the name is free, the old sidecar can be restored and the
proposal returned to review. If a competing destination exists, it is left
untouched and the original remains at its recovery name. No cleanup deletes an
unlocked pathname. Staging filenames include a timestamp so a preserved failed
stage cannot be silently reused by a later attempt.

On abrupt process exit, restoration does not run. An interrupted save can leave
the sidecar absent and the old version at its recorded recovery name. The journal
retains both complete versions and blocks automatic retry. If a current file is
present, a fresh review may explicitly **Keep current version** and retain all
recovery files/snapshots. If it is absent, the app does not automatically restore
or apply a guessed winner: manual recovery-original inspection/restoration is
still required. The recorded backup name is an intended/observed operation name,
not proof that a file currently exists there.

File sync and FULL SQLite sync are not proof of cross-store namespace durability
under power failure. Installed Windows/NTFS external-editor and power-loss trials
remain separate gates. Original media binding is a volume/file identity binding,
not a persisted content checksum of the source media between proposal and Apply.
That historical step-2 limitation is superseded in local source 0.56.71 by
[full-source checksum binding](XMP_SOURCE_CONTENT_VALIDATION.md); the saved
0.56.70 installer still has the earlier behavior.

## Step 1 findings resolved

- Hash-damaged rows carry individual integrity errors; healthy reviews remain
  accessible. Corrupt snapshots are retained and cannot be applied.
- Cancellation is journal-only. It checks workspace ownership and the selected
  candidate hash but does not open media/folders or require the root to be online.
  Removing the folder from an otherwise saved workspace or moving the photo does
  not prevent cancellation. Deleting the whole workspace does not grant another
  workspace ownership of its journal records.
- **Show completed saves and decisions** exposes completed and archived records.
  Inspection reports a current file as missing, changed, matching the saved
  candidate, or inaccessible. Intentional deletion is never treated as permission
  to resurrect a sidecar.

The pending check uses source file identity and a case-insensitive path check
within the workspace, preventing ordinary Windows path aliases from bypassing
an unresolved operation. Existing legacy `.openphoto.tmp` / `.openphoto.bak`
entries are preserved and block Apply; old snapshots are not promoted by filename
or timestamp alone. Generated recovery originals from acknowledged saves are
kept as history and do not block a new explicitly reviewed proposal.

## Verification

- 310 frontend tests passed across 65 files; TypeScript/Vite production build
  passed with the existing large-bundle warnings.
- 313 native tests passed; eight explicit acceptance tests remain skipped.
- Warnings-as-errors Clippy and Rust formatting passed.
- Tests cover successful reviewed merges and third-party XML preservation,
  exact physical original retention, stale sidecar/candidate/source identities,
  writer-first refusal, removed registration, missing-destination creation races,
  legacy recovery refusal and case-alias pending checks.
- A separate process attempts both a writable open and ReplaceFileW at the
  production Apply boundary. Windows refuses both; the exact old bytes survive
  as the recovery original.
- A live parent-directory junction conversion is attempted at that boundary
  with a valid copied junction buffer. Local NTFS refuses it with error 145
  (directory not empty); the external target and its source/sidecar remain intact.
  This is specific evidence, not exhaustive validation of arbitrary reparse tags,
  third-party filesystem filters or all storage configurations.
- Fault injection spans intent commit, original move, stage create/write/flush/
  reopening, publication and acknowledgement. Separate processes exit abruptly
  at key boundaries; cold reopening twice preserves ambiguous pending operations
  and never automatically overwrites or restores a chosen version.
- Browser fixtures check confirmation, Apply bindings, independent corrupt-row
  handling, cancellation, missing-file history and light/dark 1220×780 and dark
  800×580 layouts without horizontal overflow. These are browser fixtures, not
  installed-WebView2 acceptance.

Local evidence: `../audit-output/xmp-step2-native-final.log`,
`xmp-step2-clippy-final.log`, `xmp-step2-frontend.log`, `xmp-step2-build.log`,
`xmp-step2-live-reparse.log`, and `xmp-step2-*.png` screenshots.

## Next boundary

Step 3 should expand adversarial real-editor/interruption trials and the manual
recovery experience. Step 4 remains full installer and installed-app acceptance.
The historical ignored successful-publish race fixture still characterizes the
old test-only unsafe replacement helper; it must not be cited as a passing test
of the new runtime path. New production-boundary tests exercise the reviewed
handle-move/no-overwrite protocol. Issue #90 remains open pending those release
and durability gates. No unverified change has been shipped.

## Step 3 update — November 6

[Step 3 validation](XMP_STEP_3_VALIDATION.md) supersedes two earlier boundaries:
the successful-publication race gate has been adapted to this runtime protocol
and activated, and verified interrupted originals can now be explicitly restored
through a fresh, guarded, no-overwrite action. The real ExifTool matrix passed.
Missing/unverifiable recovery data still requires manual inspection. The save's
absence interval, installed-app acceptance and power-loss limits remain.
