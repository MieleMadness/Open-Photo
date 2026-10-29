# Build versioning policy

Open-Photo uses MAJOR.MINOR.PATCH across package.json, both dependency lockfiles, src-tauri/Cargo.toml, and src-tauri/tauri.conf.json.

1. Inspect the newest commit and completed Windows Desktop Build on the current PR branch before making changes.
2. Increment PATCH for each development release update, as required by AGENTS.md and the project owner's development instructions. A green previous run does not authorize an automatic minor-version bump.
3. Keep version-bearing files and the normal Windows workflow name/run-name aligned in one finalized commit.
4. Diagnose failed jobs and fix root causes without weakening formatting, linting, or tests.
5. Verify the workflow for the final pushed commit before declaring that build green.
6. Reserve a MINOR milestone or MAJOR public release for an explicitly chosen release scope. The signed public updater accepts only deliberate newer N.0.0 releases; development patch builds do not change that policy.
