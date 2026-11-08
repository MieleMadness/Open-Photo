# Local library update handoff — 0.56.86

Prepared November 6, 2026. Work remains local in `integration-source`. The starting checkout already contained uncommitted Phase J work; the concurrent 0.56.85 decoder changes were retained. No commit, push, merge, installation or publication was performed for this update.

## Roadmap status

| Step | Local result | Remaining boundary |
|---|---|---|
| 1 · Library correctness | Implemented: content-versioned image/HEIC/video previews, scoped analysis replies/errors/busy state, root/directory folder selection and an explicit all-folders reset. | Installed real-library acceptance. |
| 2 · Accessibility and recovery | Implemented: named keyboard map controls, a paged coordinate list and focus restoration; typed settings validation and preserved-data recovery with explicit validated repairs. | Installed Windows screen-reader acceptance. |
| 3 · Preview/performance | Implemented: shared preview deduplication/concurrency/cache service, one disposable HEIC worker, pre-allocation pixel guards and thumbnail-sized output; linear GPS-year derivation. | Whole-app peak memory, large real libraries, removable-drive behavior and HEIC variant coverage. |
| 4 · Architecture/design | Extracted settings, preview and scoped-task ownership from App; explicit folder domain model; token-based map styling, library-owned new styles, restrained tile hover and corrected capability documentation. All five accents checked in both themes. | Further architecture/CSS work can proceed incrementally; this is not a rewrite of the remaining App coordinator. |
| 5 · Release candidate | Fresh NSIS installer built; optimized short synthetic export, CPU AI, sampled-video AI and ExifTool compatibility acceptance passed locally. | Disposable-runner install/upgrade/uninstall/purge, installed UI/hardware checks, exact-commit remote CI, signing and signed upgrade/channel publication. |

## Verification

| Check | Result |
|---|---|
| Frontend suite | **361 passed**, 73 files |
| TypeScript / production build | Passed |
| Native suite | **423 passed**, 8 opt-in modes skipped by the default suite |
| Rust formatting / denied-warning Clippy / diff whitespace | Passed |
| Explicit bundled CPU AI acceptance | Passed; seven assets verified; synthetic semantic, face and OCR inference; source unchanged |
| Explicit sampled-video AI acceptance | Passed; real bundled CPU model, staged work/cancellation/resume and original-byte preservation |
| Explicit real ExifTool matrix | Passed; competing editor changes, guarded Apply/publication and restoration preserve winning edits and source media |
| Explicit optimized synthetic video acceptance | Passed; full decode, stream validation, safe publication, original bytes and temporary cleanup |
| Production-asset browser fixtures | Passed; real HEIC under production CSP, light/dark and minimum desktop window, 150% rem text scaling, ten palette combinations, keyboard map/list selection, restored focus and explicit settings repair |
| HEIC worker lifetime | One worker observed, none retained after completion; service tests additionally cover deduplication, bounded concurrency and canceled-slot ownership |
| Pattern detector | `[]`; a narrow source-pattern result, not accessibility certification |

The four explicitly selected acceptance tests above are additional to the default native suite. The four still-unselected modes concern supplied real footage, hour-scale seeking, long-recording and full-recording acceptance. No personal media was supplied or processed. Installed lifecycle/UI, GPU behavior, physical-drive faults and signed-upgrade acceptance remain separate gates.

The final optimized synthetic export used an eight-second 1080p fixture, trimmed from 0.37 to 7.61 seconds. Export wall time was **2.64 seconds**. The largest individual native child's sampled peak working set was **94,322,688 bytes**. That is not combined process-tree memory or whole-app memory, and is not a speed guarantee for real footage.

The actual `mapCaptureYears` helper processed 50,000 photos and 50,000 GPS paths in ten Node runs: median **34.9 ms**, maximum **64.9 ms** on this machine. These figures exclude filesystem scanning, React rendering and map tiles. Full-content source verification deliberately reads original bytes and can add disk work for large video previews; it has not been weakened to recover speed.

Production assets: main JS **644.70 kB / 192.36 kB gzip**; separate HEIC worker **3,215.86 kB**. The worker loads on demand. The existing large-chunk advisory remains visible rather than being suppressed.

## Installer and evidence

Installer: `../../integration-native-target/release/bundle/nsis/Open-Photo_0.56.86_x64-setup.exe`

- Size: **679,798,844 bytes** (648.31 MiB).
- SHA-256: `a11a907feb0c6c3a44cffc60a360641b3c71c90d477d19363e6403a7a9cbb95c`.
- Authenticode status: **NotSigned**. This local artifact is not a signed update-channel release.
- Packaging verified all seven pinned AI assets and prepared eleven application-local native runtime files.

`../../audit-output/library-update-integrity.json` records the installer identity and SHA-256 values for 278 source/build-input files. It identifies the dirty local state rather than claiming a new committed SHA or remote CI result.

Logs and captures are under `../../audit-output/library-update-*`: final frontend/native checks, Clippy/formatting, installer build, CPU/video AI, ExifTool matrix, browser JSON/screenshots, palette contrast, actual-helper benchmark and `library-update-video-acceptance/` reports. Browser media is synthetic except the documented public decoder fixture; originals and personal settings were not used.

## Next release checkpoint

1. Review the local diff with the retained Phase J files identified separately. Use [LIBRARY_RELIABILITY.md](LIBRARY_RELIABILITY.md) as the current behavior matrix.
2. Run the installer lifecycle script on the disposable GitHub-hosted Windows runner required by its guard. It intentionally installs/uninstalls and tests app-data removal; its guard was preserved and the script was not run on the personal profile.
3. Record installed Windows keyboard/screen-reader, representative media/drive and signed-upgrade acceptance. No signing public/private keys were configured in this local session.
4. Obtain the repository's exact-commit remote CI and full release gates before shipping. No GitHub run was triggered or inspected for this local update.
