# Open-Photo Development Instructions

These instructions apply to all work in this repository.

1. Review the relevant existing code and architecture before making changes.
2. Implement complete solutions rather than temporary workarounds.
3. Maintain compatibility with existing photo, AI, and video functionality.
4. Run all relevant tests, linting, and build checks before committing.
5. Fix identified test and compilation failures before pushing.
6. Increment the patch version for each release update and keep workflow naming aligned with the version.
7. Update README.md and CHANGELOG.md when functionality changes.
8. After pushing, check GitHub Actions and inspect any failed jobs.
9. If a workflow fails, identify its root cause, implement a fix, and verify again.
10. Report what changed, what tests passed, what remains incomplete, and the GitHub Actions status. Never claim success without verification.

## Parallel development and verification

11. For independent roadmap features, create focused branches from `feature/desktop-v0.1.0`, and open separate draft PRs. Do not wait for each PR's remote CI run to finish before implementing a different, nonconflicting feature. Keep roughly three independent in-flight feature PRs maximum.
12. Prefer nonoverlapping file ownership. Stack a PR on another only when it requires that parent's unmerged API or schema; document the dependency. Recreate/test a clean integration branch on the **latest** development baseline before merging related changes.
13. Distinguish local tests, fast GitHub CI (frontend + native), full installer validation (`full-windows-ci`), and installed-Windows hands-on QA. Never describe fast-only green as proof of installer or UI acceptance.
14. Track failures by PR, exact head SHA and first failing step. A red CI run blocks merging **that PR**, not starting a separate independent task. Fix CI failures on the owning branch at the next engineering checkpoint; canceled/superseded runs are not failures. No unverified feature may be shipped.
15. Release and integration gates remain strict: run all relevant local checks before pushing, obtain exact-head green CI before merging, and full Windows packaging/acceptance before releasing the integrated commit. Never modify source media, bypass source verification, or drop no-overwrite export guarantees to make a test pass.
16. When handing off work, report a compact queue of PR links, status, dependencies, blocking test failures, and next actions. See `docs/PARALLEL_AI_DEVELOPMENT.md` for the detailed process. GitHub CI runs asynchronously, but a chat session does not automatically resume coding after it ends.
