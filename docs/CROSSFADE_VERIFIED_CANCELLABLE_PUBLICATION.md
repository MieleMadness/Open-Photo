# Crossfade verified and cancellable staged publication

Test-only crossfade publication now composes the already-validated 64 KiB bounded/cancellable staging copy with SHA-256 scratch/stage verification, original source revalidation and the existing no-overwrite output publisher.

The order is: reject existing/unsafe output; hash private scratch MP4; revalidate original project media; open unique owned staging file; copy with cancellation and exact expected byte count; sync staged bytes; hash staged output; revalidate originals; re-hash scratch; check cancellation; atomically commit without overwriting. The RAII staged-output guard deletes temporary output on failure.

Existing `publish_verified` test callers keep their original API via a no-cancel wrapper. The source-bound `publish_with_sources` now invokes the cancellable version. Cancellation is cooperative, and a blocked filesystem write may not respond until control returns. This is still `#[cfg(test)]`: real Windows file-handle ownership, stable original-to-normalized provenance, installed-app testing and long-form resource budgets remain necessary.
