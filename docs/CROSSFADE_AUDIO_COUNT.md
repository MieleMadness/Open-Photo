# Experimental exact decoded audio clock

The native crossfade preflight predicts exactly `expected_audio_samples = expected_frames × 48000 / fps`. A new test-only checker queries bundled `ffprobe -show_frames -show_entries frame=nb_samples` on an MP4 and sums **decoded audio frame sample counts**, rather than accepting only a rounded AAC stream duration or AAC packet count. The existing cancellable native runner limits the probe output to 4 MiB, and the checker rejects invalid, zero, oversized or missing decoded frames, count overflow, excessive padding and truncation.

This utility is **not yet wired into the experimental executor** and may uncover legitimate AAC priming/padding behavior in real fixtures; it must be tested with the actual bundled FFmpeg and source timing before being used as an export gate. It does not modify production previews or exports. The full-output decode check still applies separately.
