# XMP source-content checks — local 0.56.71

New proposals store a SHA-256 checksum of the **entire original media file** in
the XMP operation journal. This complements the existing Windows volume/file
identity, root binding and sidecar identity/content checks. An in-place edit with
the same length and modification time must no longer pass as the reviewed source.

The checksum is read through the protected, read-only source handle after a
candidate is prepared, before its journal commit. Foreign/already-tagged sidecars
are skipped before this read. Review and Apply/restore independently hash the
current source through a new protected handle. The approval fingerprint includes
the recorded and observed checksums. Apply and restoration require a match; the
source handle continues excluding ordinary writers and replacements until the
operation finishes. Source files are never opened for writing by the app.

Hashing streams in 64 KiB chunks with bounded memory. It reads all bytes on each
comparison/Apply/restore and can add noticeable I/O for large videos or batches.
There is no timestamp-only shortcut, sampled hash or persisted path-only cache.
An unreadable or inconsistent-length read refuses the operation.

## Old proposals and interrupted saves

The additive, idempotent schema migration adds a nullable `source_hash` column.
It does **not** backfill an old proposal from today's file. Older proposals with
no checksum remain inspectable and cancellable, but Apply and restoration are
blocked. Cancel and create a fresh proposal to bind to the current source.
For interrupted records, preserve every recovery file and inspect manually if
the binding cannot be established; cancellation of interrupted saves is not
introduced by this change. Keeping a present current sidecar remains an explicit
fresh decision and never writes media or restores a backup.

## Local verification

Evidence is stored beside the checkout under `audit-output/xmp-source-content-*`.
The native regressions cover same-identity/size/timestamp edits, persistence on
cold reopen, stale and fresh Apply refusal, cancellation/reproposal, missing
checksum migration, restore refusal, streaming across buffer boundaries and
retained source sharing protection through publication. Check the logs for final
test counts and outcomes.

Final local results:

- **326 default native tests passed**, eight explicit acceptance modes ignored.
  The ExifTool 13.61 matrix was separately selected and passed; seven other
  explicit modes remain unrun. Six new source-content regressions are included.
- **315 frontend tests passed** across 65 files; TypeScript/Vite build passed.
  The existing bundle-size warning remains.
- Clippy with warnings denied, Rust formatting and diff whitespace checks passed.
- All six generated PNGs decoded through Windows imaging; the source-change
  pair has equal length and different hashes. All generated XMP parsed as XML.
  Thirteen fixture-file hashes verified, and creation into an existing directory
  was refused without modifying them.
- The fixture package is `audit-output/xmp-fixtures-0.56.71.zip` beside the
  checkout. Its SHA-256 is
  `06160daba4cbd74145c7702a04269106469cbcc453f9000d630cecd5235804b7`.

The first frontend run flagged the synthetic journal filename as application
data because its inventory scanner recognizes explicit `#[cfg(test)]` module
boundaries. The new module now declares that boundary separately from its
Windows restriction; the final full frontend run passed. Installer data
ownership and purge behavior were not expanded to include test databases.

## Reproducible fixtures

From the source checkout, with Node installed:

```powershell
node scripts/prepare-xmp-test-fixtures.mjs ../audit-output/xmp-fixtures-0.56.71
node scripts/prepare-xmp-test-fixtures.mjs --verify ../audit-output/xmp-fixtures-0.56.71
```

Creation requires a new directory and never replaces or deletes existing files.
Use a different directory if this one already exists. The generated README
describes new, managed, foreign, competing-editor, source-change and legacy
recovery cases. All media is generated 2×2 PNG; a valid same-size variant supports
the deliberate source-change case. The file manifest records originals, sidecars
and auxiliary files; verification is expected to fail after deliberate edits.
No application database, installer or personal profile is touched. These files
do not fabricate an interrupted journal or substitute for real-media testing.

## Packaging and remaining limits

The preserved **0.56.70 installer and its handoff snapshot do not include this
change**. Version 0.56.71 is local source work, with no replacement installer or
remote CI/release. Installed WebView2, AI/runtime, lifecycle and real-media
acceptance remain pending.

This closes the ordinary between-session source-content detection gap; it does
not establish hardware power-loss durability or protection against every storage
filter, kernel writer or pre-existing writable memory mapping. The two-phase
sidecar save can still briefly leave its destination name absent. No competing
sidecar overwrite or automatic recovery winner is introduced.
