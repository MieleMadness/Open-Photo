# Phase K: private vault design and implementation plan

Status: independent 0.56.94 review candidate on development baseline 6f91e5e965f7510399300a07183b6c03f16adce1. It excludes the unmerged library/Phase J work in PR #122 and the local Phase J production checkpoint. Default release access remains gated; independent security and installed hands-on acceptance remain pending.

## Delivery

1. Native foundation: Windows CNG AES-256-GCM and PBKDF2-HMAC-SHA256, user-scope DPAPI key protection, versioned encrypted catalog, durable throttling and short-lived native unlock sessions. Refuse unknown/corrupt formats without reset.
2. Complete local workflow: setup/unlock, copy-only encrypted photo/video import, authenticated range playback without plaintext files, lock/cancel/timeout/workspace/Windows-session handling, PIN change and encrypted backup/restore.
3. Verification: cryptographic known-answer/tamper tests, process/restart and failure cases, source preservation, access while locked, index/cache exclusion, frontend behavior and batched light/dark/narrow visual review. Release activation remains gated on security review and disposable installed acceptance.

## Threat model and decisions

Protect stored media and names against casual browsing, locked-app access and copied vault files without both the PIN and Windows user's DPAPI credentials. A hidden version gesture is discovery only. Native commands and media requests enforce authorization. This does not defend against malware/admin/debuggers running as the same Windows user, compromised application code, screen capture, browser/GPU memory, pagefile or a rolled-back filesystem. Windows full-disk encryption complements this design. Do not claim independent audit or absolute memory erasure.

Use Microsoft CNG primitives rather than custom cryptography. AES-256-GCM uses a fresh 96-bit nonce for each catalog/key wrap and a random per-file 256-bit key with unique chunk counters. Chunk associated data binds the vault ID, object ID, total plaintext length and index. Private keys and app-owned plaintext work buffers use zeroizing owners. Framework-owned WebView response buffers are outside that erasure guarantee. CNG/DPAPI output buffers are cleared before freeing. PINs never enter preferences/logs. Numeric PINs require at least six digits; passphrases require at least twelve characters. Paste/password managers remain allowed.

PBKDF2-HMAC-SHA256 uses 600,000 iterations and a random 256-bit salt. PIN-derived AES wraps a DPAPI-protected random master key. DPAPI uses the current user and UI-forbidden flags, never machine-wide scope. The master key encrypts the catalog, which includes opaque object IDs, original names, media types, per-file keys, byte lengths and plaintext digests. No private metadata or original paths are stored outside ciphertext. Object size/count, filesystem timestamps and format/version remain observable.

Use an isolated native-owned `Open-Photo/private-vault-v1` directory. A bounded versioned state file contains only crypto parameters and ciphertext. Object files have opaque random names. Native file handles reject reparse points/hard links and prevent concurrent source writers. A process lease serializes vault access. Short native metadata read/publication sections are coordinated so Windows cannot see an open metadata reader during replacement; callers receive complete byte snapshots and perform crypto outside that section. Atomic, synced state replacement preserves the previous complete state on failure. Imports stage ciphertext, verify complete decryption/digest, publish without overwrite, then atomically publish the encrypted catalog. Crashes may leave inaccessible encrypted orphans; never report an uncommitted import as success or silently delete user data.

Each native unlock session uses an unpredictable token, current workspace identity, native monotonic inactivity timing and a cancellation epoch. Native lock invalidates tokens before workers can deliver or publish, clears owned keys and emits a lock event. A native watchdog checks Windows session state and five-minute inactivity; media polling does not extend the session. Every media response rechecks authorization and carries no-store/no-referrer headers. Frontend lock clears passwords/media URLs and unmounts the viewer. Closing the vault, application closure and workspace changes lock it.

Imports are copy-only. Supported first-version previews use original JPEG/PNG/WebP/GIF/BMP and WebView-compatible MP4/WebM videos; files and image dimensions are bounded. Unsupported playback produces a clear error and preserves the encrypted copy. No FFmpeg scratch, persistent plaintext thumbnails, AI, face/OCR indexing, recent-project entries or automatic decrypted exports are permitted. Normal scanning explicitly excludes the private directory, including ancestor scans. Shared normal output writers reject vault destinations after canonicalizing their parent; only a native store-created ciphertext target can use the encrypted-object output path. Decrypted export/delete and full master-key rotation are not exposed in this version.

Recovery is intentionally account-bound: encrypted backup/restore needs the same Windows account's DPAPI credentials and PIN. There is no bypass/reset/wipe for a forgotten PIN. Restore refuses an existing vault and validates all chunks before publishing a complete new vault. PIN change atomically rewraps the same master key and invalidates prior sessions; a crash leaves either old or new credentials valid. Normal uninstall and ordinary app-data cleanup retain the encrypted vault. Setup explains these limitations before the first import.

## Sources

- [Microsoft authenticated cipher parameters](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/ns-bcrypt-bcrypt_authenticated_cipher_mode_info)
- [Microsoft BCryptDecrypt](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptdecrypt)
- [OWASP password storage and PBKDF2 parameters](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)
- [Microsoft PBKDF2 API](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptderivekeypbkdf2)
- [Microsoft Windows session state](https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/ns-wtsapi32-wtsinfoex_level1_w)
- [Microsoft user-scope DPAPI example](https://learn.microsoft.com/en-us/windows/win32/SecCrypto/example-c-program-using-cryptprotectdata)


## Implemented workflow and limits

The entry remains on the version control, including the collapsed sidebar. Keyboard Enter/Space opens the same single native-modal surface as three rapid clicks. Setup and restoration explain account-bound recovery. Password managers and paste are allowed; credentials, private filenames and media URLs never enter settings/localStorage. The dialog clears credentials and unmounts private media when locked or closed; native notifications and a non-renewing authorization check catch idle/workspace/session revocation. Only actual user interaction renews the inactivity clock. Windows-account identity comes from native DPAPI, and workspace identity comes from saved native settings.

The first version supports 500 items and 5 GB total. Photos are JPEG/PNG/WebP/GIF/BMP, up to 64 MB and 50 megapixels; videos are MP4/WebM up to 512 MB. Codec compatibility is delegated to the installed WebView. Private media responses authenticate each requested range, decrypt and verify whole affected 1 MB chunks, return at most 8 MB for video or 64 MB for a photo, and recheck authorization before delivery. A two-worker gate bounds concurrent media response buffers. Malformed/out-of-bounds authenticated ranges return 416; unauthorized requests return empty 404 responses. No persistent plaintext media, FFmpeg scratch, ordinary media-protocol grants or private thumbnail cache is created.

Lock invalidates access immediately; already-running OS I/O may finish before a worker observes cancellation. It cannot deliver stale plaintext or report an unpublished import as saved. Plaintext necessarily exists transiently in native, WebView and GPU memory while viewing; zeroizing owned native buffers and clearing DOM state are not a claim of full OS/browser memory erasure.

Encrypted backups use `.opvault`. Backups authenticate every copied encrypted object in the closed staged archive before no-overwrite publication. Verification checks the exact header/length, every authenticated chunk, full plaintext digests and archive EOF with chunk-sized buffers. Restore requires an empty vault location and verifies the complete archive before publishing a new directory. Unknown versions, directory junctions, file hard links, damaged envelopes, incomplete archives and mismatched authentication tags are refused without replacing existing data. PIN change is an atomic key rewrap, not a full media/master-key rotation.

Ordinary app-data cleanup intentionally leaves `private-vault-v1` and `vault-attempts-v1.json`. The disposable installer script now contains a vault-preservation fixture, but has not been run for this version. Default release access is disabled; Windows debug or explicit `npm run tauri -- build --features private-vault` acceptance builds enable the vault. This is a local preview, not a security certification or permission to bypass release gates.

## Design review

| Before | After | Why |
| --- | --- | --- |
| The version was a passive footer and disappeared with the collapsed rail | Version control has triple-click and Enter/Space access in both layouts | Keep the requested discreet entry accessible without another navigation tab |
| No private storage boundary | Native encrypted catalog, authenticated object chunks and revocable sessions | UI visibility cannot grant filesystem/media access |
| Password inputs inherited browser-default dimensions | Password fields share the existing text/search/number tokens, focus and hover rules | Preserve readable controls and theme consistency |
| Normal viewers use persistent preview infrastructure | The vault viewer uses an authenticated no-store range channel | Avoid writing decrypted media into ordinary caches |


## Independent candidate verification — 2026-11-08

| Check | Result | Local evidence |
| --- | --- | --- |
| Native suite on independent baseline | 306 passed, 9 skipped (including child-only crash worker); all 31 vault tests included | `../../audit-output/vault-review-native.log` |
| Frontend suite on independent baseline | 317 passed across 66 files | `../../audit-output/vault-review-frontend.log` |
| Clippy and explicit vault feature compilation | Passed | `../../audit-output/vault-review-clippy.log`, `vault-review-feature.log` |
| TypeScript, production build, formatting and whitespace | Passed; existing chunk-size advisory remains | `../../audit-output/vault-review-types.log`, `vault-review-build.log`, `vault-review-format.log` |
| Isolated browser fixtures | Light/dark, narrow, setup/view/lock, Unicode limits, focus and no ordinary provider calls passed; mocked native IPC/media | `../../audit-output/vault-review-browser.json` |
| Installer/native acceptance preparation | PowerShell syntax parsed; requires disposable hosted runner execution | `scripts/test-installed-app.ps1` |

These counts differ from the historical mixed-source results below because the independent candidate excludes unmerged library/editor/test additions. Tests were not removed from the vault. The original mixed working tree is preserved unchanged. The separate local `feature/phase-j-production-0.56.88` checkpoint remains dependent on PR #122 and is not part of this PR.

Full CI now compiles `private-vault` explicitly and, on the guarded disposable installed runner, builds a separate acceptance-only release test executable with that feature. It runs native vault tests beside installed DLLs using synthetic roots, archives its log/JSON, and checks vault plus throttle-state retention after update/default uninstall/explicit ordinary-data cleanup. The installed app itself still uses the default closed gate. Native acceptance is not WebView/GUI/cache/playback sign-off. See the PR body/checks for the exact remote head and observed results.

## Historical combined-source verification — 2026-11-07

| Check | Result | Local evidence |
| --- | --- | --- |
| Full native suite, 0.56.93 | 497 passed; 9 skipped (8 opt-in modes and the child-only crash worker) | `../../audit-output/phase-k-backup-native-final.log` |
| Vault cases within the final full native suite | 31 passed; child-only worker invoked by the parent | `../../audit-output/phase-k-backup-native-final.log` |
| Full frontend suite, unchanged since 0.56.92 | 413 passed across 78 files | `../../audit-output/phase-k-mime-frontend.log` |
| Latest vault frontend checks | 15 passed | `../../audit-output/phase-k-mime-ui.log` |
| Clippy, all targets, warnings denied | Passed | `../../audit-output/phase-k-backup-clippy.log` |
| Explicit private-vault acceptance feature compilation | Passed | `../../audit-output/phase-k-backup-feature.log` |
| TypeScript and production Vite build | Passed; existing large-chunk advisory remains | `../../audit-output/phase-k-backup-types.log`, `phase-k-backup-build.log` |
| Rust formatting and diff whitespace | Passed | `../../audit-output/phase-k-backup-format.log` |
| Browser layout and interaction fixtures, unchanged since 0.56.92 | Setup/empty/populated/wrong-PIN; light/dark at 1220×780 and 800×580; no page errors/horizontal overflow; 42px themed password inputs; keyboard entry/focus restoration; oversized Unicode credentials blocked before native setup; lock clears private DOM; no normal preview/AI provider calls | `../../audit-output/phase-k-browser.json`, `phase-k-*.png` |
| Disposable installer preservation script | Syntax parsed; not executed | `scripts/test-installed-app.ps1` |

Native coverage includes AES-256-GCM/PBKDF2 known answers, DPAPI roundtrip/tamper, chunk ranges and authenticated HTTP headers, old-token rejection, wrong-PIN backoff across restart, original preservation, ciphertext swaps, malformed/future formats, process leases, junction/hardlink refusal, backup integrity/empty-only restore, PIN rewrap, native lock/idle/workspace revocation, interrupted import, poisoned-session key cleanup, racing destinations and the ordinary-writer/private-storage boundary. Frontend coverage includes native lock confirmation failure, StrictMode listener disposal and obsolete authorization polls during PIN rotation.

Parallel checks exposed Windows access-denied failures at metadata/directory publication. Metadata now uses coordinated short reads that release file handles before publication, with a multi-reader/multi-publisher snapshot regression. Atomic publication retries only observed Windows busy/denied conditions (5/32/33), at most seven attempts over 315 ms of backoff, and checks cancellation before each syscall. Overwrite collisions, integrity checks, crypto and source copies are never retried; persistent errors preserve existing data. The cause of every external Windows access-denied event is not asserted. Installed filesystem/antivirus/sharing acceptance remains required. Final local suites above pass.

Browser evidence uses isolated mocked native IPC and synthetic media. It establishes interface behavior/layout, not installed cryptographic or playback acceptance. Native crypto/storage/protocol tests use disposable synthetic files and the current test account's DPAPI; they never initialize the personal vault or install the app.

## Local follow-up review — 0.56.90

This is an implementation review, not independent security sign-off. The default release gate stays closed.

| Before | After | Why |
| --- | --- | --- |
| Authentication captured cancellation state after waiting for the native operation lock | Native dispatch captures the generation before queuing; setup/unlock/restore retain that admission generation | Lock must cancel work that has not started as well as running work |
| An authentication request waiting for workspace settings could still invoke native code after close | A request-validity check runs after persistence and before invocation | Prevent canceled UI work from starting a fresh native unlock |
| A stale workspace/watchdog observation could globally lock a replacement session | Revocation compares the observed generation; every successful grant advances it | A delayed observer has authority only over the session it checked |
| Delayed lock events and obsolete authentication cleanup were global | Lock notices carry the last revoked generation; grants carry their generation; obsolete replies revoke only their token | Preserve newer unlocks while rejecting responses already covered by a lock |
| Lock invalidated atomics before waiting for session ownership, then cleared whichever session it found | Cleanup removes only sessions at or below the revoked generation | A newer grant acquired while cleanup waits must not be silently cleared |
| CNG authentication metadata was passed through a readonly pointer cast | Provider metadata is mutable; empty AAD uses a null pointer | Match the native API's in/out structure contract |

The native regression suite exercises queued setup/unlock, queued restore from a valid backup, obsolete token/generation revocations, and delayed workspace checks in operation, lease and watchdog paths. Frontend regressions cover cancellation while settings persist, verified lock acknowledgements, delayed notices and revoked pending grants. Existing PIN change, crypto known-answer, tamper, original-preservation and no-overwrite tests remain required.

The CNG structure contract is documented by [Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/bcrypt/ns-bcrypt-bcrypt_authenticated_cipher_mode_info). This change corrects the FFI contract; it does not assert that every single-shot provider invocation previously wrote those fields.

## Recovery acceptance follow-up — 0.56.91

A cold-process parent test now exercises eleven test-only failure points: partial encrypted import, published object before catalog, published catalog, before/after PIN wrapper publication, partial/before/after backup publication, and partial/before/after restore directory publication. Each child exits without Rust destructors; its lifetime is bounded and its root must be a marked synthetic directory directly beneath the test temp directory. No hooks or environment-controlled process exits are compiled into the application.

After restart, the catalog must be complete, the applicable old/new PIN must work, existing media must decrypt to original bytes, and unpublished backup/restore destinations must stay absent. Retry into the empty restore destination must work. Encrypted staging/orphan objects may survive an abrupt exit; they are preserved and never silently adopted or automatically reclaimed. This is process-exit recovery evidence, not a power-cut or disk-full test.

A separate persistent-sharing test holds the real Windows metadata file open against replacement. Failed import and PIN rewrap must leave the catalog bytes, current PIN and source unchanged, remove handled staging, and permit import once the obstruction is released. This supplements the bounded atomic-retry unit test without relaxing file sharing or no-overwrite guarantees.

The [disposable installed-Windows matrix](PHASE_K_WINDOWS_ACCEPTANCE.md) makes the remaining hands-on checks concrete. It is prepared, not executed.

## Image typing and credential consistency — 0.56.92

| Before | After | Why |
| --- | --- | --- |
| Image dimensions came from a guessed header, while the MIME type came from the extension | New imports derive the MIME type from the verified supported image header | A PNG named `.jpg` must be served as PNG; original names/bytes remain unchanged |
| TIFF/AVIF content renamed with an accepted extension could pass dimension checks | Only JPEG/PNG/WebP/GIF/BMP headers are accepted for photo imports | Keep the actual imported content within the documented private viewer format set |
| Frontend maximum length counted UTF-16 units; native validation counted UTF-8 bytes | Both enforce the native byte budget; oversized input gets a shortening instruction | Unicode credentials must not pass creation validation then fail solely because of different length units |

Regression coverage verifies PNG/JPEG/BMP with mismatched extensions, private response Content-Type and original bytes, unsupported TIFF and malformed inputs without publication, media types after encrypted backup/restore, and accented/emoji credentials at/beyond 256 UTF-8 bytes. Existing encrypted catalogs are preserved without automatic metadata migration; the typing change applies to new imports. Video playback compatibility remains an installed WebView check.

## Closed archive verification — 0.56.93

| Before | After | Why |
| --- | --- | --- |
| Backup authenticated source objects, copied their ciphertext, then checked archive length | Backup copies from immutable source handles and authenticates the closed staged archive before publication | Verify the artifact being saved, including copied bytes and header, rather than relying only on source validation and output length |
| Object verification repeatedly opened ranges within a standalone file | A shared sequential verifier authenticates one complete object from a bounded stream | Use the same chunk/AAD/digest checks for individual files and objects embedded in an archive |

The v1 format is unchanged. Source copy and archive authentication remain two streaming passes; no plaintext file or full-archive memory buffer is introduced. Owned decryption buffers zeroize on drop. Failed verification leaves the staged output uncommittable, and publication still refuses existing destinations. The final native authorization check is retained before publication. Regression cases alter archive magic/length/header/ciphertext/tags, truncate/append data, and cancel between chunks; each failed verification must prevent commit and preserve existing backups and sources.

## Remaining release gates and queue

1. Independently review the threat model, FFI/crypto format, PIN/recovery policy, metadata publication and plaintext lifetimes. Keep default release activation disabled.
2. Review the independent vault commits and obtain exact-head CI plus full Windows packaging. Earlier [PR #122](https://github.com/MieleMadness/Open-Photo/pull/122) is a separate library/Phase J candidate; its prior CI does not validate this branch. Record current results in this candidate’s PR body/checks.
3. On disposable installed Windows, verify actual WebView range seeking/audio, native file pickers, OS lock/disconnect, inactivity, keyboard/screen-reader behavior, multi-instance handling and profile/account-bound restoration. Inspect actual WebView cache/temp behavior and test cold crashes, no-space and persistent filesystem/antivirus denials.
4. Run prepared installer upgrade/default-uninstall/explicit-cleanup retention checks, including encrypted vault preservation, then signed upgrade acceptance before release activation.

Phase J release validation remains paused and its local changes are retained. Portable recovery, decrypted export/delete, HEIC/MOV conversion inside the vault and full master-key rotation require separate design/implementation; they are not exposed by this first version.
