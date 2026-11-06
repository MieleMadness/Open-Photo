# Experimental verified crossfade render-to-publication chain

This **test-only** pipeline combines existing crossfade safeguards in execution order:

1. Verify **previously stored** original-media fingerprints (SHA-256 and length).
2. Confirm canonical scratch ownership and normalized MKV stream/frame metadata, including decoded input video frame count.
3. Execute the bounded, cancellable FFmpeg crossfade graph and independently probe plus fully decode its MP4.
4. Recheck original-media fingerprints after encoding.
5. Validate exact AAC **presentation** sample clock against the planned frame clock, accounting for encoder priming and packet padding.
6. Recheck original-media fingerprints before and after copying to a uniquely staged output, then commit with existing no-overwrite semantics.
7. Remove the scratch MP4 on either success or failure; only a fully published MP4 is retained.

A bundled-Windows fixture tests two 60-frame 30fps synthetic normalized clips with a 15-frame overlap and 105 final frames (3.5s), plus final decode, audio presentation timing, SHA-256 source expectations, publication collision, changed originals and scratch cleanup.

**Not production enabled:** The test uses synthetic sources and trusted local scratch paths. Real sequence snapshots, photo/video source normalization, Windows reparse-point/TOCTOU protection, disk reservation, progress reporting, secure cancellation during very long copies, bounded-memory audio packet inspection, installed-app acceptance and persisted transition UI remain required. The current saved project schema, production export and existing hard-cut/dip renderer are unchanged.
