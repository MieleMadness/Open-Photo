# Audit integration — 0.56.67

Combined from desktop c67dc56e (0.56.64), in version order:

- PR #83, a58e1fff: shared AI source versions, cache/publication/queue/search freshness and legacy migration (0.56.65).
- PR #84, f14934b5: SQLite-to-memory settings hydration and exact-value retries without browser-storage quota dependency (0.56.66).
- PR #85, 2a8caa1f: literal map tooltip labels (0.56.67).

The final application version, npm/Cargo lockfiles, Tauri configuration and Windows workflow title/run name are 0.56.67. Changelog entries and README notes retain all three patches. Original patch reports are historical; this integration is the combined review surface.

## Verification

Combined local checks: 64 frontend files / 301 tests passed, TypeScript and production build passed, and formatting passed with the Rust version used by CI. Existing bundle-size warnings remain. Native code from PR #83 is preserved alongside the already merged RAW/exit/purge changes from #82.

Exact-head Windows CI must pass frontend, Rust formatting, Clippy, native regression, short real-model video, installer build, release-export measurement, installed AI and uninstall acceptance before this integration merges. CI results are recorded on the integration PR to avoid changing tested source merely to update status text.

This merges development code only. No release tag, signed channel publication, or installed GUI/upgrade acceptance is implied. Manual close/update, quota recovery and realistic RAW checks remain separate. Full-content source hashing adds disk reads and has no large-library latency guarantee.

## Remaining audit priorities

1. XMP recovery states and external-editor conflict verification (F07/R01).
2. Installed hands-on safety acceptance and native registered-root/junction/revocation checks (R02/R03).
3. Consistent backup/restore, remaining documentation and dependency/release hardening.
4. Service extraction and measured polling/cache/search/resource improvements.

Do not resume vault/Immich or major editor expansion before the stability gates are demonstrated.
