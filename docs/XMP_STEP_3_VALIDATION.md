# XMP step 3 — editor, interruption and explicit recovery validation

Local checkpoint: November 6, 2026. No code was uploaded, no installer was built,
and the installed app was not changed. This document extends the step 2
[reviewed Apply protocol](XMP_REVIEWED_APPLY.md).

## Real metadata-editor trials

ExifTool 13.61 ran successfully from its
[official tagged source](https://github.com/exiftool/exiftool/tree/13.61), commit
`9b8664dfcf2c609d3cc8925d380e98ad2a379f9b`, using the Perl runtime already installed
with Git for Windows. User ExifTool configuration was disabled with `-config ''`.
Native verbatim drive paths were normalized for normal command-line addressing.
There is no new application dependency or bundled editor.

The SourceForge portable ZIP attempts returned HTML download pages, not ZIPs;
none was extracted or executed. The official source checkout provided the
working alternative. The editor modified only synthetic XMP fixtures; original
media bodies were disposable sentinel bytes whose preservation was checked.
These are real editor/NTFS metadata trials, not photo-decoder or installed-GUI
acceptance tests.

The explicit editor matrix passed:

- Edits before Apply invalidate the earlier approval and remain byte-identical.
- Both `-overwrite_original` replacement and `-overwrite_original_in_place`
  writes are refused while the old sidecar is protected. The original survives
  as the exact physical recovery file after the reviewed merge.
- A new ExifTool sidecar created after the protected original moves wins the
  publication race. Open-Photo refuses to overwrite it or restore over it.
- An ExifTool edit after candidate publication survives acknowledgement, and
  completed-save inspection reports that it differs from the saved candidate.
- An ExifTool sidecar created during explicit original restoration also wins.
  The old recovery original remains unchanged; a subsequent fresh Keep current
  decision changes only the journal.

The test is opt-in because a verified editor runtime is not a build dependency.
It was explicitly run and passed, rather than counted as covered by default CI.

## Active publication race gate

`xmp_publish_race_tests::external_edit_between_compare_and_successful_publish_must_not_be_lost`
now runs against the production reviewed-Apply path on Windows and is no longer
ignored. The external edit still happens **after the authoritative comparison
and before candidate publication**. In the two-phase protocol it occupies the
now-free sidecar name after the original moved; the final no-overwrite
publication must fail and preserve the externally published bytes. The exact
old sidecar and source media must survive as well.

The seam was adapted to the new protocol; the competing edit was not removed.
The legacy test-only unprotected replacement helpers are not used by the runtime
and are not being described as safe. Other editor/handle tests exercise refusal
before a protected old pathname can be modified.

## Explicit original restoration

Interrupted Apply can leave a missing sidecar and the original under the recorded
private recovery filename. Tags now offers **Restore saved original** only when:

- The operation is pending/interrupted (`applying`, `backed_up`, or `restoring`).
- The destination is absent and source/folder identities still match the record.
- The recovery filename matches this operation's generated private-name format.
- The opened recovery file is regular, not a reparse entry or hardlink alias,
  size-bounded, and matches both the original file identity and SHA-256 hash.
- The journal snapshots pass integrity checks and no legacy temp/backup collision
  needs manual review.

Inspection binds the recovery file's identity/hash/status into the approval
fingerprint. Restoration reopens it with write/delete sharing denied and verifies
that it is exactly the version reviewed. The original snapshot is expanded in
the UI and a separate confirmation checkbox is required before the action.

A FULL-sync `restoring` intent precedes the filesystem operation. Authorization
is checked again while source, directory and recovery handles are retained. A
native handle-relative **no-overwrite** rename returns the verified original to
the empty sidecar name. Any competing destination wins; no replacement or copy
fallback is attempted. `restored` is acknowledged only after the rename succeeds.
Original and proposed journal bytes remain available in completed history. The
physical backup name is consumed by a successful restoration, and the UI treats
its recorded name as historical reference rather than an extant file promise.

An interrupted restoration is also explicit: after fresh inspection, a missing
destination with an intact verified backup can be restored again. If the original
already reached the destination before acknowledgement, the user can freshly
Keep current instead. Repeated requests never automatically select a version or
overwrite a file. Bad, moved, aliased, replaced or unverifiable backups remain
blocked and preserved for manual inspection.

The tag browser refreshes after Apply, restoration or acceptance of a current external version. Late results from a different workspace do not trigger metadata refresh; this does not reintroduce automatic library scanning.

The UI now refreshes the operation state from the snapshot response, so stale
list data cannot keep presenting normal proposal controls for an interrupted
save/restoration.

## Failure and restart coverage

Recovery fixtures now begin with an actual child process exiting abruptly after
Apply moved the original. Further child exits cover restoration intent commit,
pre-move, post-move and pre-acknowledgement boundaries. Cold SQLite reopening
twice does not promote, clean up or choose a version. A fresh user decision
completes the recovery while both journal versions remain intact.

Synthetic failures cover the same transitions; mutation, same-content identity
replacement, missing/aliased recovery files and untrusted filename cases refuse
restoration. A competing-file restore race preserves the winner and backup.
These are abrupt-process tests and deterministic injected failures, not a test
of sudden hardware power loss or every storage/permission configuration.

The full run also exposed an older test-fixture cleanup issue: file-backed test
connections were still open when temporary directories were removed. Stale
folders later collided when Windows reused a process ID. The fixture now closes
the database before cleanup and exclusively creates/skips existing names.
Existing failed-run evidence is not deleted to make tests pass. This was a test
lifetime defect, not evidence of a production XMP overwrite.

## Verified local results

- **315 frontend tests** passed across 65 files; TypeScript/Vite build passed.
  Existing bundle-size warnings remain.
- **320 native tests** passed in the full default run, with eight explicit modes
  ignored. One of those eight is the new editor matrix, which was separately
  selected and **passed**. The other seven existing explicit acceptance modes
  were not run. Do not report the default skip as editor acceptance.
- Clippy with warnings denied, Rust formatting and diff whitespace checks passed.
- Browser fixtures passed restoration confirmation, missing-destination behavior,
  fresh approval binding, completed recovery history and light/dark 1220×780 and
  dark 800×580 layouts without horizontal overflow. They are not installed
  WebView2 acceptance.

Evidence in the workspace's `audit-output` folder:

- `xmp-step3-native-final.log`
- `xmp-step3-exiftool.log`
- `xmp-step3-clippy.log`
- `xmp-step3-frontend.log`, `xmp-step3-build.log`
- `xmp-step3-restore-*.png`, `xmp-step3-restored-history.png`

The test editor checkout and failed download pages live under
`audit-tools/exiftool-step3`, outside the application source and package assets.

## Step 4 boundary

Ready for a local installer build and installed-Windows acceptance. Validate the
actual packaged WebView2 flow, real supported photo/video originals, install /
uninstall and permission/storage failures against the exact built source.
The two-phase save still has a possible absent-name interval. Source identity
binding still does not persist a checksum of every source media byte between
proposal and Apply. Power-loss durability, unsupported storage and arbitrary
filesystem filter/reparse behavior have not been established. Issue #90 is not
closed and no release or remote CI success is claimed.
