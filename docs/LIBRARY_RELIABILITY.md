# Library reliability: current capability and acceptance

Local candidate **0.56.86** includes the library repair work and retains the separately developed, test-only Phase J 0.56.85 decoder groundwork. This page is the current status reference; earlier milestone notes describe their historical implementation.

## Current behavior

| Area | Behavior | Acceptance boundary |
|---|---|---|
| Discovery | Startup restores the saved index. Manual Sync discovers changes. Adding a folder imports it. Watchers and periodic timers remain disabled. | Local regression coverage; large removable-drive and physical-failure acceptance remain separate. |
| Folder browsing | Each row represents one source root and directory. Selection shows files directly in that directory; nested folders have separate rows. Show all folders removes the filter. | Covers root/nested, neighboring names, Unicode, verbatim/UNC, case/separator differences and distinct roots. |
| Analysis | Duplicate, similarity and health tasks bind replies/errors/busy state to their request and workspace generation. Sync and root/workspace changes invalidate prior work. Health reports name the checked folder. | Local delayed-reply and generation tests; duplicate review remains read-only. |
| Preview identity | Renderer cache keys and native image/video cache namespaces include content identity. Native requests recheck the expected identity and verify it again before returning newly generated or cached output. Unversioned legacy renderer results are not retained in the renderer cache. | Same-size and preserved-time image/video changes tested with real decoded output and original-byte checks. |
| HEIC preview | One shared service allows three active previews overall and one HEIC decoder. Each HEIC worker has a 30-second deadline, dimension checks before RGBA allocation, a 1,920px output-edge ceiling and an 8 MB encoded-output cap. Disposal terminates the worker. | Positive real-HEIC decode under production CSP plus queue/limit/cancellation tests. This does not certify every HEIC variant or a whole-process memory ceiling. |
| Settings | Organizational JSON is validated in a typed repository. Invalid/future data blocks app rendering and remains preserved. Recovery can save the original, validate an explicit repair and await its durable save. Existing bare-value storage remains supported. | Wrong types/null entries/future envelopes, legacy/v1 reads and explicit-repair UI tests. Native previous-value history also retains the replaced value. |
| Map | Named keyboard marker actions and a paged/filterable coordinate list open the same media. Focus returns after File Details closes. Year lookup uses a path set rather than nested searches. Marker colors use theme/accent roles. | Browser keyboard/cluster checks and 50k-item derivation coverage. Installed Windows screen-reader/hardware acceptance remains pending. |
| Video editor | Existing sequencing, continuous preview/export and non-destructive source handling remain. Phase J composed preview, title/audio mixing and production crossfade routing remain gated. | Retained native/frontend regressions; see VIDEO_ROADMAP.md for feature gates. |
| Updates | Signing remains required. A local unsigned candidate is not an update-channel release. | Signed bootstrap/upgrade and channel publication remain pending. |

## Ownership and design

- `settingsRepository.ts` owns structured-setting types, legacy/v1 validation and explicit repair. `userData.ts` continues to own serialized durable writes and shutdown protection.
- `useScopedTask.ts` owns request lifetime and result ownership. `libraryFolderSelection.ts` owns Windows folder identity and direct-child selection.
- `LibraryThumbnail.tsx`, `previewService.ts`, and the HEIC worker own library thumbnail presentation, scheduling and decoder lifetime. Native `preview_source.rs` owns expected source-version checks shared by image/HEIC/video paths.
- `tokens.css` owns map color roles; `library.css` owns the new location-list/recovery surfaces. Existing foundations and editor layout are preserved. Frequent tile browsing no longer translates tiles on hover.

## Verification record

Evidence is stored outside the repository in `../../audit-output/library-update-*`. Final counts and installer identity are recorded in [the 0.56.86 handoff](LOCAL_LIBRARY_HANDOFF_0_56_86.md). Passing local tests is not an installed acceptance claim.

The browser fixture uses the public libheif `examples/example.heic` from the upstream repository solely as a local decoder test input, downloaded into audit-output, not bundled with Open-Photo. Its SHA-256 is `7f8b363e4936c0666a25f64f3a92fda10bd8e5453be4592530b65a55dd98f3f2`. The image decoded to a 360×240 thumbnail under the production CSP. Source: [libheif examples](https://github.com/strukturag/libheif/tree/master/examples). Other UI media is synthetic.

## Release gates

1. Pass the maintained frontend suite, TypeScript/production build, formatting, denied-warning Clippy and native suite for the final local state.
2. Verify browser light/dark, minimum-window layout, all accents, text scaling, keyboard map selection/focus, recovery and HEIC decoding using the actual production assets/CSP.
3. Build a fresh NSIS installer with the reviewed bundled AI/video/native-runtime resources, and record its path, size and SHA-256. Packaging does not install the app.
4. Run `scripts/accept-video.ps1` for the optimized synthetic native pipeline. Explicit AI/XMP acceptance modes have distinct evidence; skipped modes remain listed.
5. Run `scripts/test-installed-app.ps1` only on its required disposable GitHub-hosted Windows runner. Preserve its guard: it intentionally installs/uninstalls and tests app-data removal. Do not spoof runner variables or run it on a personal profile.
6. Obtain exact-commit remote CI, installed UI/screen-reader/hardware acceptance and signed upgrade evidence before publication. This local-only update does not push, install, merge or publish.

Broad architectural rewrites, GPU/HEIC variant certification, long-recording acceptance and physical-drive fault validation are not implied by this repair pass.
