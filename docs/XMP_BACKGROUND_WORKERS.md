# Background XMP work — local 0.56.72

XMP commands now dispatch SQLite access, full-source checksums, file guards and
publication to Tauri blocking workers. The asynchronous command waits for the
worker result; it does not run a filesystem read or wait for a save lock on the
WebView command thread. This follows the existing local AI/video command pattern.
All six existing command names, request fields and serialized result shapes stay
the same, so existing frontend `invoke` calls continue awaiting a final result.

Review/list commands use the common worker dispatcher. Tag creation, Apply,
restore and review decisions additionally share the existing XMP write lock,
acquired **inside** the worker. Serializing decisions matters now that commands
can run concurrently: cancellation must not slip between the committed intent
for a new sidecar and its no-overwrite publication. A decision that reaches an
already completed save is refused rather than retroactively cancelling it.
The lock is not a FIFO ordering promise. Source/root/version checks still run
when each operation actually executes.

Each worker owns its database connections and protected file handles. Ordinary
validation errors are returned unchanged. An unexpected worker failure returns
an error instructing the user to refresh saved changes; it never reports a save
as successful or selects an automatic recovery winner. If a panic poisons the
write lock, subsequent writes fail closed with a restart/review instruction.

Once an explicitly confirmed write starts, losing its reply or closing a view
does not interrupt the worker midway through a transaction. It can still finish
and acknowledge the save. Reopen/refresh the journal to inspect the actual result
before retrying. This is not a new cancel/pause/progress feature. A process exit
still requires the existing persisted-interruption recovery workflow.

## Verification and limits

Native worker tests use synthetic media and their own disposable SQLite files:

- A channel-gated slow operation returns `Pending` to its caller and runs on a
  different thread; another task can complete while it waits.
- Validation errors return unchanged rather than becoming success values.
- Dropping the reply future after submission still lets a confirmed Apply finish;
  cold reopening shows the candidate, acknowledged state and exact retained old
  sidecar, with source bytes unchanged.
- A worker panic after the original move produces a failure result. Two database
  reopens preserve the interrupted state and backup without automatic restoration.
  This test isolates the dispatcher from the global write lock to avoid poisoning
  other tests; actual write-lock poisoning deliberately requires app restart.
- New-sidecar publication retains the write lock across journal commit and
  publication, and review decisions use that same lock. A later cancellation of
  the completed save is refused.

Final local validation passed: **331 native tests**, **315 frontend tests**
across 65 files, TypeScript/Vite build, Clippy with warnings denied, Rust
formatting and diff whitespace checks. Five worker tests are included in the
native total. Eight explicit acceptance modes remain ignored in this run; the
real ExifTool matrix last passed separately against the 0.56.71 core and was not
rerun for this dispatch-only update. The existing frontend bundle-size warning
remains.

Evidence is stored in `audit-output/xmp-worker-*` beside the source checkout.
These are backend
scheduling and filesystem tests, not measured installed-WebView2 responsiveness.
Large-file hashing still reads the entire source and can take time; there is no
promise that an XMP result returns immediately. Database/handle contention can
still refuse an operation safely.

The full-source checksum and no-overwrite/recovery invariants from
[0.56.71](XMP_SOURCE_CONTENT_VALIDATION.md) remain in force. No installer is rebuilt,
installed or uploaded; the preserved 0.56.70 installer does not include these
source updates. Installed real-media, lifecycle and power-loss acceptance, remote
CI and release remain pending. No Issue #90 closure is claimed.
