# Phase J delivery plan

## 1. Composed visual previews — implemented as a gated native prototype

Combine verified decoded photo/video frames using the existing frame clock's dissolve and dip weights. Preserve source pins and stale-request checks through final delivery. Add bounded pixel composition, then connect it to decoded inputs and test actual transition frames, cancellation and preview/export parity. Titles and audio remain for phase 2.

The RGB kernel is now connected to verified real decoding and the session frame clock through `compose_next`. Single frames, sample-centered dissolves and discrete-frame dips produce bounded typed RGB output. Source pins remain owned by the result; the active worker slot is retained until composition completes. Supersession cancels obsolete delivery, and `visual_current` rechecks session/project identity before consumption. No UI or production route is activated.

Real video/photo fixtures cover single and overlap frames, dip endpoints, unchanged originals, retained source locks, supersession during composition and final stale delivery. Bundled FFmpeg's explicit RGB blend reference agrees within one channel level (integer rounding). This establishes visual-kernel reference parity, not equivalence to the old experimental YUV `xfade` export graph. Phase 3 must adopt/reconcile this explicit RGB, sample-centered policy and validate actual end-to-end exports before activation. Titles and audio are intentionally absent until phase 2.

Exit gate: verified clock-driven single/overlap/dip frames, preserved originals, bounded resources, rejected stale delivery and real-engine pixel parity.

## 2. Titles and audio — implemented as a gated native prototype

Render global titles in the agreed composition order. Decode and mix exact PCM windows with gain, mute, fades and transition envelopes. Resolve the existing linear/equal-power policy explicitly. Add visual and listening/parity tests while preserving phase 1 ownership and cancellation guarantees.

The gated implementation now provides `av_next`/`av_current`, post-transition global titles and exact 48 kHz stereo windows. It adopts sample-centered linear envelopes, sums before final clipping and exposes clipping counts. Real-engine tests cover timing, positions, source pins, mute/fades, overlapping linked clips, repeated sources, cancellation and 30/60-fps sample boundaries. See [titles/audio policy and verification](PHASE_J_TITLES_AUDIO.md). Digital reference checks are implemented; physical listening and actual production export-route parity remain phase 3 acceptance work rather than being claimed from unit tests.

Exit gate: title positioning and sample-accurate audio agree between preview and export, with clipping, cancellation and boundary tests.

## 3. Editor integration and release validation — in progress

Connect composed previews to the shared editor and production scheduler. Complete native workspace authorization, multi-root relinking, versioned storage/migration/import, recovery and old-client handling. Validate real footage, rapid seeking, missing sources, installer lifecycle and installed Windows behavior before enabling production crossfades.

Started with a read-only native workspace-admission bridge over the actual project service, binding full document/local-source metadata and checking saved-root ownership at admission and final revalidation. It supports multi-root snapshots and preserves future-schema records. This is test-only groundwork, not production activation. See [phase 3 progress and remaining gates](PHASE_J_PRODUCTION_INTEGRATION.md).

Exit gate: exact-commit CI, installed acceptance and preview/export parity. Keep existing hard-cut/dip production behavior until these gates pass.
