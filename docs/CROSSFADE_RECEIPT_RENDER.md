# Experimental receipt-guarded crossfade rendering

A test-only `render_verify_publish_snapshot_receipts` entrypoint now combines:

1. A validated saved source manifest (video/photo/standalone audio identity), with no re-learned original hashes.
2. The **exact saved** per-clip frame allocation and frame rate.
3. Canonical owned scratch paths and distinct normalized MKVs.
4. Ordered normalized receipts matching the saved timeline (clip/source IDs, persisted source SHA-256, MKV SHA-256 and frame count).
5. Revalidation of **both original media and normalized MKV receipt hashes** before and after FFmpeg runs, and another receipt check after AAC presentation verification before staged publication.

The original `render_verify_publish` and `render_verify_publish_snapshot` test-only APIs remain available for their existing synthetic regression fixtures, while the new entrypoint refuses swapped receipts without leaving a published MP4.

**Trust boundary:** Receipt fields remain public within test-only Rust modules, so an untrusted caller could forge a consistent receipt for an unrelated normalized clip. The verifier checks *identity consistency and byte stability*, not authorship. Only the internal trusted normalizer should construct these records in future production use; pin source/part handles and validate the real installed Windows workflow before connecting the feature to the editor.
