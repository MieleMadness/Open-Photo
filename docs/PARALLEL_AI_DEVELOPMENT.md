# Parallel AI development workflow

Status: Team workflow, November 2026. Applies to Open-Photo development on `feature/desktop-v0.1.0`, not release policy.

## Objective

Keep developing independent features while GitHub Actions verifies previous work. A green result is required **before integration**, not before beginning an unrelated feature. Avoid waiting for each full Windows installer build just to begin coding the next patch.

## Branch strategy

- **Stable development baseline:** `feature/desktop-v0.1.0`. Keep it free of unfinished changes.
- **Independent features:** branch directly from the current baseline as `feature/<focused-change>`. Each PR targets the baseline and uses fast CI.
- **Dependent features:** stack on one feature PR only when they truly need unmerged APIs/schema. Name the dependency explicitly in both PR descriptions. Never quietly merge a dependent PR into the baseline before its parent.
- **Integration branch:** assemble a small, related group of features against the current baseline and validate the combined exact commit. Don't silently overwrite current workflow files with older copies from a feature branch.
- **Docs:** separate branches/PRs are welcome; do not block unrelated feature coding on documentation-only tests.

## Parallel work queue: working agreement

Keep a maximum of about **three independent PRs in flight** per AI development session, plus one integration PR if necessary. Avoid changes to the same files across branches when possible.

1. **Build** a focused feature and its tests. Run available static/local checks before pushing.
2. **Push** and open a draft PR. Record exact commit SHA and the expected acceptance criteria.
3. **Move to the next independent feature immediately** while fast CI runs.
4. **Check previous results at natural checkpoints** (before an integration, after another PR is committed, or on the user's next request). Log red failures in the queue. Do not abandon or merge failing work.
5. **Fix red tests in the owning PR** when returning to it. Re-run validation for its new exact SHA; prevent stale CI results from being mistaken for latest status.
6. **Integrate after green checks.** If two features touch shared timeline/renderer/schema code, validate the combined branch, not only individual green PRs.
7. **Run full Windows installer acceptance** using the `full-windows-ci` label, a manual dispatch, or release/main pipeline on the actual integration commit. For interactive editor changes, installed-app hands-on QA is an additional, separate gate.

## CI response rules

- **Fast green:** npm tests/build + Rust fmt/Clippy/native regression checks passed, no installer guarantee.
- **Full green:** Windows packaging, video acceptance and installed/uninstall acceptance passed automatically. Manual UI checks are *still* required for interactive features.
- **Red:** capture first failing step, log and exact SHA; identify code failure vs infrastructure/network/cache failure. Fix on the feature branch, not with an unreviewed workaround on the baseline.
- **Canceled:** concurrency cancellation by a newer commit is not a test failure.
- **Running/queued:** no success/failure claim yet.
- **Skipped installer:** intentional in fast PR mode, not evidence of full release validation.

## Queue / handoff format

Record the following for each active PR in an issue, PR body or session update:

| Field | Example |
| --- | --- |
| Feature | Previous / Next Cut navigation |
| Branch / PR | `feature/timeline-cut-navigation-05668` / #16 |
| Exact head SHA | Record after latest push |
| CI | pending / fast green / red (step) / full green |
| Blocker | Missing source, failed test, UX acceptance, or none |
| Next action | Fix failing test; test in Windows; integrate |
| Dependency | PR #15 or independent baseline |

**Priority when re-entering development:** critical failures affecting baseline or data safety first; then failed integration PRs; then independent feature progress; then lower-priority styling/documentation. Failures may wait while unrelated feature work proceeds, but no failing feature may be merged, called complete or shipped.

## Current feature sequence

- PR #15: combined direct audio trims, video dip-to-black, adjustable durations and native preview/export parity. Fast CI passed on the integration SHA. Request full Windows verification and manual editor QA before integration.
- PR #16: Previous/Next Cut timeline navigation; stacked on PR #15 and needs exact-head validation. Next candidate for integration after parent settles.
- Additional nonconflicting timeline polish can develop on separate branches while these checks run.
- Phase I next renderer-level goal: real crossfade and audio transition design **after** current timing, persistence and playback behavior is accepted. This requires coordinated TS/Rust tests.
- Phase G 10,000-media stress test remains deferred to the pre-1.0 acceptance checklist.

## Boundaries for AI assistance

AI can write and push several branches/PRs in one active session and inspect/fix build results in later sessions. GitHub Actions executes asynchronously in GitHub itself, but a chat response does **not** launch an autonomous agent that will monitor future runs and edit code without another invocation. Use Codex/long-running agent work only when deliberately set up; never claim CI failures will automatically be fixed in the future without a real configured automation.

## Principles

Preserve source media, original-file checksums, workspace permissions, migration paths, and no-overwrite export policy. Keep UX and rendering changes tied to regression tests. Choose small reviewable commits and a regular integration cadence, not one installer run per individual button.
