# Technical audit remediation — 0.56.64

Baseline: active desktop branch 86367d3 (0.56.63), not the older audited af856ef checkout.

## First patch

- F01: common close/update barrier drains settings before project discard/save and rejects new settings writes until the operation finishes. Persistence errors retain the app and permit retry. Regression coverage includes delayed saves, failed-save/project-discard, retry and restart hydration.
- F03: embedded RAW extraction checks encoded length and dimensions before reading preview bytes. In-memory decoding uses the same dimension, pixel and allocation limits as file decoding. RAW orientation stays authoritative; embedded EXIF is not applied twice. Native regressions cover oversized headers and all eight embedded EXIF orientations.
- F04/C01: purge includes ai-work.sqlite and SQLite companions. A source-derived inventory regression compares production database names with uninstall cleanup and installed acceptance. Actual installed lifecycle acceptance remains required.
- F05: current baseline already retains the callback in a ref and has clustering work beyond the audited version; do not overwrite it with the old implementation.

- C02: prepared Windows native CI explicitly selects the short ignored real-model video staging/resume test; long/manual tests remain excluded.

## Verification boundaries

Frontend suite, TypeScript and production build run locally using existing dependencies. Cargo is unavailable locally; native formatting, Clippy and tests require exact-head Windows CI. Full installer acceptance and hands-on Windows close/update/RAW checks remain required before release. No release tag or publication is part of this patch.

## Remaining order

1. F02: unified source identity and migrations across queue, semantic/OCR/face stores and search freshness. Include preserved timestamps, same-second rewrites, legacy record invalidation and publication fences.
2. F06: remove startup dependence on LocalStorage capacity and retry from in-memory values.
3. F07/R01: explicit XMP recovery states, fault injection and external-process conflicts.
5. Reassess remaining documentation, authorization, junction, cache and polling findings against the current branch before changing them.

Version policy: major releases increment the first number; features the second; bug fixes the third. Workflow title and run name must match the application version. This bug-fix batch is 0.56.64.
