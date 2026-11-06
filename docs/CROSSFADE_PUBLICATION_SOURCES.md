# Test-only crossfade publication with original source integrity

The experimental verified-publication path now has a strongly coupled adapter, `crossfade_publish::publish_with_sources`. It takes **stored** file-length and SHA-256 expectations for the original media, not newly learned hashes. The adapter reuses the existing cancellable `crossfade_integrity::verify_source_fingerprints` helper at both verification boundaries in `publish_verified`: before copying the fully validated scratch MP4 and immediately before its no-overwrite commit. A cancelled or replaced original rejects the publication and the staged temporary output is cleaned up.

Unit regression uses a previously fingerprinted original file, demonstrates cancellation and same-size mutation rejection, then publishes after restoring the original, and refuses a second publication to the same destination.

This code remains test-only. A production caller must still snapshot/relink validated media, render and fully decode verified MP4 and AAC presentation samples, enforce scratch ownership, control filesystem replacement races, and guarantee cancellation and disk budgets during large copies. No user-visible crossfade export route has been added.
