# Build versioning policy

Open-Photo uses MAJOR.MINOR.PATCH across package.json, src-tauri/Cargo.toml, and src-tauri/tauri.conf.json.

Before preparing a new development build:
1. Inspect the most recent completed Windows Desktop Build on this PR branch.
2. If it failed, diagnose the failing log and include a real fix in the next commit. Advance the PATCH only (e.g. 0.1.0 -> 0.1.1). Never intentionally weaken Clippy, formatting, or tests to make a build green.
3. If it succeeded, increment MINOR and reset PATCH to 0 (e.g. 0.1.2 -> 0.2.0).
4. Increment MAJOR only as part of an explicitly approved main release.
5. Keep all three version-bearing files identical in the same finalized build.
6. Wait for CI before declaring the release candidate green. A successful older workflow is not proof that a later commit passes.

Automated CI checks that version-bearing files agree; the choice of next version is made after inspecting the preceding workflow, because multiple GitHub push events and concurrent PR runs can otherwise race.
