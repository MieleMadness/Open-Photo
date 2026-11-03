# Local AI search, duplicate detection and GPU acceleration

## User goals
- Find media by phrases such as "red shirt", "sunset on the beach" or a person's name.
- Detect byte-identical duplicates and visually similar photos/videos.
- Keep models and embeddings entirely on-device unless the user explicitly opts into external services.
- Use GPU acceleration when available, but fall back gracefully to CPU.

## Layered design

### Phase 1: Exact duplicates (no AI/GPU necessary)
Compute streamed SHA-256 fingerprints with an incremental SQLite index; filter by size first; verify hashes before making recommendations. Never automatically delete files or collapse distinct workspace references.

### Phase 2: Perceptual near-duplicates
Compute normalized pHash/dHash for supported photos (and a sampled video frame). Rank suggestions by Hamming distance; allow user review and mark "keep both". Multiple resized/edited versions are not the same as byte-identical duplicates.

### Phase 3: Semantic local search
Use a CLIP-like vision/text embedding model to encode photo thumbnails and user search queries into the same vector space. Add a local vector index scoped to workspaces. Combine semantic relevance with filename, EXIF, GPS, dates and user-entered tags. Results should explain why they matched.

### Phase 4: GPU-optional execution
Probe available acceleration (Windows DirectML, ONNX Runtime CUDA where compatible, CPU fallback). Do not assume a GPU or download large models without permission. Record model version, embedding dimension, provider, and deterministic index migration strategy. GPU helps image embedding generation; exact duplicate detection generally needs no GPU.

### Phase 5: Video + iPhone media
Decode keyframes with a supported media backend (FFmpeg licensing review and shipping strategy needed), honor video rotation and timestamps, and index semantic embeddings for representative frames. Handle HEIC/HEVC and Live Photo image/video pairs.

## Operational constraints
- Background queues must be cancelable, resumable, bounded in memory, and prioritized behind interactive browsing.
- Re-scan based on path/size/modification timestamp and verify hashes when needed.
- Store workspace membership separately from physical file identity.
- Never export face embeddings or precise GPS locations by default.
- No auto-deletion or automatic person merging.
- Ship pinned models and redistribution notices with the installer; verify build downloads and installed assets. Runtime does not download models.

## Test plan
- Hash comparisons across identical and altered fixtures.
- pHash false-positive and false-negative fixtures.
- Text search retrieval examples, including clothing colors and negations.
- CPU-vs-GPU similarity tolerance, cancellation, model change/migration and degraded-mode tests.
- Multi-root workspace isolation, overlap handling, inaccessible folders, missing files and rescan resilience.

## Status

### Implemented
- Exact SHA-256 duplicates and local visual near-duplicate review.
- Supported OpenCLIP ViT-B/32 image + text inference.
- Persistent semantic image embeddings and natural-language search.
- Incremental/full-library AI indexing with stale-embedding skips.
- Windows DirectML-first execution in Automatic mode with CPU fallback.
- Bounded frontend AI batches with visible progress and safe cancellation between batches.
- Installer-bundled, checksum-verified local model storage and an AI on/off switch.
- Backend-derived compatibility metadata and Settings readiness states for Semantic Search, People and OCR.
- Frontend gating so incomplete or incompatible model bundles cannot start AI jobs.
- Phase 3 self-check command/UI for model bundle readiness, DirectML availability and explicit real-world validation status.
- Pinned YuNet 2026may + SFace 2021dec People model contracts.
- Local face detection, five-landmark ArcFace alignment, 128-value SFace embeddings, model-provenance persistence, incremental People indexing and suggested local grouping UI.
- Pinned English PP-OCR detector + recognizer contracts, local OCR region extraction, persistent text indexing and local photo-text search.

### Still required before Phase 3 is fully validated
- Validate the pinned YuNet + SFace People pipeline on representative real libraries and refine grouping thresholds without silently merging identities.
- Validate PP-OCR accuracy and region post-processing on screenshots, signs, receipts and document photos; expand languages only with separately pinned contracts.
- Validate DirectML results against CPU within documented similarity tolerances on representative Windows hardware.
- Add pause/resume and persistent resumable queues if long-running indexing moves beyond the current bounded cancel-between-batches design.
- Exercise the exact supported OpenCLIP bundle end-to-end on representative libraries before calling semantic search production-ready.

The old architecture-only status below has been superseded by the implemented milestones above.
