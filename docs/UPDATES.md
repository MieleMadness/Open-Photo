# Open-Photo stable updater

## Planned Preview channel — not implemented

The October 30 feedback adds a separate signed Preview channel for in-app updates to development patch releases. The implementation/release gates are specified in [R0 of the feedback plan](AI_VIEWER_VAULT_ROADMAP.md#r0--preview-builds-update-in-place). The current Stable-only behavior below remains the actual implementation. A one-time signed bootstrap installer may be required before existing installations can receive Preview updates. Keep the same app/data identity, preserve user data, isolate Stable and Preview manifests, retain signature and active-work protections, and verify a genuine signed upgrade between two versions before publishing the channel as ready. No keys, release workflow or update manifest were changed by this documentation update.

Open-Photo uses Tauri's signed updater for deliberately published major releases.

## Release policy

The installed app accepts only a newer `N.0.0` release.

- `1.0.0`, `2.0.0`, `3.0.0`, and later major releases are valid.
- `0.56.0`, `1.1.0`, `1.0.1`, prereleases, PR builds, and branch builds are not valid stable updater releases.
- A stable update must be built from `main` by the manual **Publish major stable update** workflow.
- The rule is enforced twice: the release workflow rejects non-major versions, and the Rust updater comparator rejects any remote release that is not a newer `N.0.0`.

## One-time signing setup

Tauri requires updater signatures and does not allow signature verification to be disabled. Generate the updater key pair on a trusted machine:

```powershell
npm run tauri signer generate -- -w "$HOME/.tauri/open-photo.key"
```

Store the private key securely outside the repository. Losing it prevents existing installations from trusting future updates.

Add these GitHub Actions secrets before the first public stable release:

- `OPENPHOTO_UPDATER_PUBLIC_KEY`
- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if the key uses a password

Normal PR and development builds do not receive these values, so the stable updater is disabled in those builds.

## Publishing 1.0.0, 2.0.0, etc.

1. Finish testing the release candidate.
2. Merge the tested code to `main`.
3. Set the repository version files to the intended major release, for example `1.0.0`, and let normal CI pass on `main`.
4. Open GitHub Actions and run **Publish major stable update** from `main`.
5. Enter the exact version and release notes.
6. The workflow repeats tests, builds the signed NSIS updater, verifies the installed app, creates `latest.json`, and publishes the GitHub Release.
7. Existing signed public installs discover the release on startup or when the user chooses **Check for updates**.

Minor and patch GitHub Releases may exist for testing or manual distribution, but they must never be uploaded to the `stable-channel` release. The app also independently rejects any manifest whose version is not `N.0.0`.

## Manifest

Signed public builds check:

`https://github.com/MieleMadness/Open-Photo/releases/download/stable-channel/latest.json`

The `stable-channel` release is a fixed machine-readable channel whose `latest.json` asset is replaced only by the major-release workflow. This keeps ordinary GitHub Releases from changing the updater endpoint. The manifest contains the release version, notes, Windows installer URL, and exact Tauri signature. Signed-version verification requires the signature to match the version announced by the manifest.

## Active-work protection (0.56.2)

Startup checking waits for the installed version and enabled channel, schedules one check per app session/version, and does not consume that check if React cancels the pending timer. Pending replies after unmount close their update resource. Checks, downloads, and installation cannot overlap.

Restart & install uses the same protection boundary as normal window closure. It blocks an active or starting video export and saves all pending project revisions before invoking the native installer. A failed save or blocked export leaves the downloaded update ready to retry. Signature verification and the major-only release policy remain unchanged. Hook tests use mocked updater events; a genuinely signed public release still requires release-channel acceptance.

The native check configures 15-second connection and 30-second stalled-read deadlines. Downloads have no fixed total transfer deadline, allowing the bundled-AI installer to complete on slower links while stalled connections fail. The check itself retains a 15-second total deadline. The native check returns Tauri update-resource metadata; the stock JavaScript SDK still downloads, verifies signatures, installs, and releases resources. The deliberate signed-release workflow now also runs optimized video export acceptance before publication.
