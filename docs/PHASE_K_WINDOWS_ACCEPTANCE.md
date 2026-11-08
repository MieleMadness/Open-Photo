# Phase K disposable Windows acceptance

Status: hands-on matrix prepared, not executed. Automated installed/native execution is tracked separately in the candidate PR checks. Local native tests and mocked browser checks do not establish installed acceptance. Default release activation stays disabled.

Use a disposable Windows VM/account with synthetic photos and videos. Record the exact source SHA, clean/dirty status, installer SHA-256, feature flags, Windows build, WebView2 version and filesystem. Use an explicit `private-vault` acceptance build. Keep that artifact separate from default release artifacts. Never install an acceptance build into a personal profile or use personal vault/media data.

## Local recovery evidence

Run `cargo test --manifest-path src-tauri/Cargo.toml --locked vault:: -- --test-threads=8` on Windows. The cold-process parent test creates synthetic disposable vaults and starts its ignored worker itself; do not invoke that worker manually. The worker exits without Rust destructors at partial-write and publication boundaries. These checks establish process-exit recovery, not power-loss durability or installed WebView behavior.

Crashes may leave encrypted staging directories/files or unreferenced ciphertext objects. Reopening must preserve them without adopting them into the catalog, resetting the vault or silently deleting recovery material. Automatic orphan reclamation is outside this version.

## Installed matrix

For each row record pass/fail, exact steps, expected/observed result and a redacted evidence location. An unexecuted row stays pending. Keep credentials, session tokens, private names and plaintext out of reports and screenshots.

| Check | Procedure | Required result |
| --- | --- | --- |
| Entry and authentication | Open with triple-click and Enter/Space in expanded/collapsed sidebar; create after recovery acknowledgement; try wrong PIN, pasted Unicode passphrases and oversized Unicode input | One protected dialog; accessible labels/focus; no private content before authentication; correction stays possible |
| Original preservation | Hash synthetic source files before/after copy, cancel, crash and restore | Every original byte remains unchanged; disclosure says external originals remain accessible |
| Native pickers | Import and choose a backup destination through actual Windows dialogs; cancel each picker | No accidental unlock/revival; cancellation has no publication; if visibility locking occurs, operation requires a fresh unlock and loses no original |
| Media | View supported photos (including supported images with renamed extensions) and MP4/WebM; seek repeatedly, play audio, switch items, test unsupported codecs | Authenticated bounded ranges work; one active player; unsupported playback retains ciphertext and explains the failure |
| Explicit lock/close | Lock during viewing/import/backup, then close the vault and the app | Media stops; names/DOM clear; native access is revoked; queued old requests never reopen it |
| Windows lifecycle | Lock/unlock Windows, disconnect/reconnect an RDP session, suspend/resume | No private content remains available after native lock/disconnect detection; fresh authentication is required |
| Inactivity and workspace | Wait five minutes without interaction while playing media, then interact in a separate run; switch workspace during copy | Playback/polling does not renew inactivity; real interaction does; workspace change revokes old access |
| Multi-instance | Launch another app process against the same disposable profile | Second vault writer is refused; first vault and originals remain intact |
| Crash recovery | Terminate the disposable app during import, PIN change, backup and restore; restart cold | Catalog is a complete old/new version; valid PIN corresponds to saved wrapper; no partial destination is reported as saved; existing vaults/backups are not overwritten |
| Space and sharing | On a disposable limited-size volume, exhaust free space during writes; hold metadata against replacement; repeat after releasing the obstruction | Operation reports failure; existing catalog/PIN, sources and destinations remain intact; no success until publication; retry works after obstruction is removed |
| Backup and restoration | Backup, restore into an empty disposable target under the same account; retry into an existing vault; alter/truncate a backup | Valid backup restores every byte; existing target stays unchanged; damaged archives are rejected without reset |
| Account boundary | Attempt restoration under a different disposable Windows account | DPAPI-protected master cannot be recovered; original encrypted backup remains unchanged |
| Cache/temp boundary | Compare disposable profile/cache/temp contents before viewing, during playback and after lock/restart | No app-created plaintext previews, media scratch, AI/index entries or decrypted backups; document actual WebView cache behavior and any unexplained artifact |
| Accessibility and themes | Keyboard-only entry, setup, viewing, PIN change, lock and close; screen reader; light/dark, display scaling and minimum supported window | Focus stays within the dialog and returns to the version control; errors/status are announced; controls remain reachable and readable |
| Installation lifecycle | Run `scripts/test-installed-app.ps1` only on its guarded disposable GitHub-hosted Windows runner; perform signed upgrade acceptance separately | Upgrade/default uninstall/explicit ordinary-data cleanup preserve encrypted vault and throttle state; full packaging and signed upgrade both have exact-artifact evidence |

## Release decision

Independent review must cover the crypto format/FFI, recovery policy, authorization, cancellation, metadata publication and plaintext lifetimes. Obtain exact-head CI, full Windows packaging, the installed matrix above and signed upgrade acceptance before enabling default release access. A local test pass, a green earlier PR or a manually enabled preview is not release sign-off.

Return findings to [the Phase K implementation and queue](PHASE_K_VAULT.md). Phase J validation remains paused.
