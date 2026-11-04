# Experimental pinned Windows handle lifecycle

The test-only `render_verify_publish_snapshot_receipts` path now holds two independently verified `PinnedMedia` guards until rendering, AAC presentation verification and staged no-overwrite publication return. One guard pins the saved source manifest; the other pins the exact ordered MKV receipts. Each file is SHA-256-verified **using the retained Windows read handle** opened with `FILE_SHARE_READ` (no write/delete sharing).

The existing path-based rechecks are still performed before and after FFmpeg and before publication. On cancellation or failure, the RAII guards automatically release all handles and the experimental scratch-output cleanup still applies. A failed guard acquisition does not invoke FFmpeg.

**Limitations:** This is test-only, not installed-application validation; it does not eliminate all pathname/reparse-point and hard-link attacks, nor does a callback-controlled receipt prove it was minted by the actual trusted normalizer. Holding read handles does not block privileged writers or guarantee behavior on remote filesystems. Windows acceptance must confirm FFmpeg can read pinned files while other writers and renames are denied, and must exercise cancellation and large projects before production crossfade routing.
