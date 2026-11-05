# Audit F06 — Settings recovery without browser quota (0.56.66)

Desktop settings now hydrate from SQLite into the renderer's live memory repository. App, navigation, browse view, timeline layout and playback preferences read through readSetting. Normal desktop writes update memory and join the ordered native save queue. Only theme/accent values of at most 256 characters are mirrored as optional bootstrap preferences.

Legacy browser values are read for native migration; inaccessible browser storage does not block native loading. Existing SQLite values still win. No ratings/collection/workspace mirror is required, and absent native keys do not fall back to stale browser keys after hydration. A failed native load leaves the prior state intact and retains the startup retry screen. A failed native save preserves the exact live value for Retry saving, independent of browser quota. Close/update protection from PR #82 remains in place.

Local verification: 64 test files / 299 tests passed; TypeScript and production build passed. Regressions include a 6 MB native setting with quota failures, inaccessible browser storage, failed-save/retry using the latest memory value, restart hydration and exit protection. Existing library, navigation, playback and editor tests pass. Windows CI and installed quota/close/update hands-on acceptance remain separate gates.

Based on the verified 0.56.64 changes from PR #82, now merged as c67dc56e. Version 0.56.66 follows independent AI freshness PR #83 (0.56.65); coordinate integration/version reconciliation before release. No release publication is included.

Remaining: runtime schemas and normalization of large settings are separate D01 work. XMP recovery states and external-editor races (F07/R01) remain next in the audit queue. This patch does not erase native metadata, change media files or bypass failed persistence.
