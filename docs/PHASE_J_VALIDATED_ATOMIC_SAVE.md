# Phase J — Validated native draft transaction (test-only)

This fixture composes previously separate Phase J foundations into one **in-memory** SQLite transaction. Inside the transaction, it reads the authoritative video sequence revision, checks the complete crossfade snapshot against an independently supplied expected source manifest (including exact rational trims), then compares the draft revision and commits either a new record or a changed record with a recovery entry.

Regression tests exercise a successful two-version update, changed source fingerprints, missing or invalid snapshot data, stale authoritative sequence revisions, invalid overlap budgets, stale draft revisions, injected recovery-history failures, and a cap of 20 retained recovery snapshots. Errors must preserve the previous complete draft bytes.

**Not a production save API:** the source manifest is passed by a test, not verified against pinned originals. The tables exist only in memory and use test-only names. No application database migration, Tauri command, schema v3 change, media I/O, user-facing crossfade, live preview or export route is added. The next production proposal needs trustworthy native source resolution, schema and old-client compatibility, durable recovery and corruption handling, installed Windows real-footage acceptance, and exact-head CI.
