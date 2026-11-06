# Phase I — Windows real-media acceptance record

This is the remaining *human-operated* gate after automated Phase I native CI. A green Actions full-installer workflow does **not** mean the installed UI can create or export crossfades: all crossfade paths are currently test-only. Record results only for features actually exposed by that build.

## Test environment

- Windows build/edition: ____________________
- Open-Photo version/commit: ____________________
- Installer workflow ID and artifact: ____________________
- GPU and driver: ____________________
- CPU, RAM and disk/filesystem (local NTFS, ReFS, network): ____________________
- System FFmpeg absent (yes/no): ____________________

## Required media set (keep originals unchanged)

- Phone SDR MP4/MOV with audible audio
- Phone portrait clip with rotation metadata
- Variable-frame-rate clip
- Clip without audio
- Two or more differently oriented clips
- Source with non-ASCII and long filename
- Relinked source, missing source and duplicate/unchanged asset

Record source hashes before and after each test. Never upload personal footage without explicit approval.

## Installed editor functionality (production hard-cut/dip fallback)

- [ ] Import multiple clips; play, trim, split, reorder, undo and redo
- [ ] Preview/project playhead boundaries agree with actual output
- [ ] Reopen saved project; make sure source identities and edits are unchanged
- [ ] Export with no global/system FFmpeg; decode output and inspect A/V presentation
- [ ] Cancel and restart export; only job-owned scratch artifacts are removed
- [ ] Existing destination collision; no overwrite or source modification
- [ ] Missing/relinked original and unauthorized output location fail safely

## Test-only crossfade pipeline (native fixture, not installed UI)

- [ ] Exact-head Rust/frontend checks are green
- [ ] Bundled FFmpeg normalizes a silent and an audible source
- [ ] Saved clip/source hashes and normalized receipt order are verified
- [ ] FFmpeg renders overlapping frames; decoded video frame count matches planned clock
- [ ] AAC presentation samples match planned audio clock
- [ ] Scratch MP4 is deleted after verified staged publication
- [ ] Second publication refuses overwrite and preserves original destination hash
- [ ] Pinned Windows read handles prevent ordinary write/delete opens throughout normalization/render
- [ ] Reparse-point and hard-link alias attempts are rejected
- [ ] Low disk-space, cancellation and process failure paths leave no partially published file

## Findings and disposition

| Date | Media/fixture | Expected | Actual | Pass/fail | GitHub issue/PR |
| --- | --- | --- | --- | --- | --- |
| | | | | | |

## Signoff rule

A CI pass establishes automated checks only. Complete manual tests, record evidence, and resolve any unsafe failures before Phase J enables persisted crossfade transitions or active editor export. Pre-1.0 physical drive-removal, actual disk-full and 10,000-item library stress remain separately tracked as requested.
