# Consolidated development candidate 0.56.95

Status: combined implementation prepared and locally tested; exact-head CI is tracked in the integration PR. This is a development review candidate, not a release.

## Inputs and ownership

| Input | Exact source | Disposition |
| --- | --- | --- |
| Development baseline | `6f91e5e965f7510399300a07183b6c03f16adce1` | Current fetched baseline |
| Library, XMP safety, title validation and preview foundation | [PR #122](https://github.com/MieleMadness/Open-Photo/pull/122), `7b9fa5586becc9dbc27040e9a77ae3909936c6a8` | Included through Phase J parent history |
| Phase J production integration | `f95bed1f7a70fe525810b10f55ed74b1947b433a` | Local source branch preserved; included here |
| Account-bound vault | [PR #123](https://github.com/MieleMadness/Open-Photo/pull/123), `c4b1860ed0aec2b1a5ebbd4a34e0c15d015fab87` | Included with independent installed/native acceptance tooling |

The original mixed working folder and both clean review branches remain unchanged. Use this integration branch for combined review. The separate PRs remain open for provenance/review; do not independently merge overlapping candidates and assume their old checks validate the result.

## Conflict resolutions

| Before | After | Why |
| --- | --- | --- |
| Separate version/feature declarations | Six synchronized version surfaces at 0.56.95; both `phase-j-preview` and `private-vault`; default features empty | One coherent candidate while preserving release gates |
| Competing App import changes | Library/settings/task ownership fixes plus the protected vault entry/mount | Keep both interfaces on the shared application shell |
| Competing trailing style blocks | XMP recovery and vault rules coexist; password controls use established tokens | Preserve recovery access and consistent controls |
| Different Windows API feature lists | Union of source ownership, CNG and Windows session APIs | Retain both native safety boundaries |
| Separate workflow revisions | Current opt-in packaging/lifecycle tooling and compilation of both acceptance features | Earlier per-branch results do not validate combined code |

No storage schema, cipher/archive format or source-media migration is introduced. Source verification, immutable original handles, authorization/cancellation and no-overwrite publication remain required.

## Verification policy and next work

At the user's November 8 request, hands-on/live application testing waits until closer to V1. This includes real-footage/listening, screen-reader/hardware behavior, Windows lifecycle, physical-drive and signed-upgrade acceptance. These checks remain on the pre-V1 release checklist and do not block ongoing implementation. Independent security review is still required before vault release activation.

For this consolidation run the full local frontend/native suites, Clippy, formatting, TypeScript, production build, acceptance-feature compilation, short mocked browser checks and exact-head fast CI. Full packaging remains opt-in; prior CI #1437 (PR #122) and #1439 (PR #123) are historical separate-candidate evidence and are not a full combined acceptance claim. No personal-profile installation or live testing is requested now.

After a green combined development checkpoint, implement full-length cached audio waveforms. The current excerpt is capped at 120 seconds. Safe Recycle Bin/undo work remains a separate later feature requiring its own source-safety design.

## Combined local evidence — 2026-11-08

| Check | Observed result | Evidence |
| --- | --- | --- |
| Full native suite | 497 passed, 9 opt-in/child-worker modes skipped | `../../audit-output/consolidated-native.log` |
| Full frontend suite | 413 passed across 78 files | `../../audit-output/consolidated-frontend.log` |
| TypeScript and production build | Passed; existing large-chunk advisory retained | `../../audit-output/consolidated-types.log`, `consolidated-build.log` |
| Formatting, whitespace and installer-script syntax | Passed | `../../audit-output/consolidated-format.log`, `consolidated-whitespace.log` |
| Automated browser fixtures | Light/dark/narrow, setup, viewing, lock cleanup, Unicode input, keyboard/focus and exclusion from ordinary providers passed; mocked IPC and synthetic media | `../../audit-output/consolidated-browser.json` |
| Source preservation | All 53 original changed source files match the combined source after conflict resolution, with normalized line endings | Original source remains in `integration-source`; no original edits were replaced |

Clippy with warnings denied and compilation with both `private-vault,phase-j-preview` enabled passed; evidence is recorded in `../../audit-output/consolidated-clippy.log` and `consolidated-feature.log`. All six version surfaces, empty default features, combined Windows APIs/CSP and the untouched original 68-file snapshot were checked. Remote fast CI is required on the resulting exact head. No hands-on testing or installer run is part of this local evidence.
