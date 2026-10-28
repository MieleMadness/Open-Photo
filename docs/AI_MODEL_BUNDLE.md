# Bundled local AI (0.54.12)

The Windows installer includes all seven supported assets: OpenCLIP image/text encoders and tokenizer, YuNet detector, SFace embedder, and English PP-OCR detector/recognizer. Settings has one Local AI switch, initially off for a new installation. Enabling AI does not download models or upload photos. Indexing remains an explicit action; Off stops future batches and retains indexes and reviewed names. A batch already running may finish. Existing GPU-first/CPU fallback and explicit CPU selection remain available under diagnostics.

Builds prepare weights from the pinned sources and SHA-256 identities in [config/ai-models.json](../config/ai-models.json). The checksums match the backend's supported model contracts. `npm run prepare:models` verifies cached/staged assets and fails on an incorrect download. Model binaries are generated build resources, not committed to Git. CI caches them by manifest identity. The installer is larger because it contains approximately 650 MB of uncompressed AI assets.

Runtime reads `bundled-ai` beside the installed executable. It verifies model identities before inference; missing or incompatible assets require repair/reinstallation rather than a download prompt. Legacy manually imported assets remain a fallback only where the corresponding bundled file is absent. Bundled and legacy directories are distinct, avoiding duplicate per-user copies of installer assets. Model inventory hashing runs on a worker thread.

The bundle retains the model card, licenses, provenance, and attribution under `bundled-ai`. See [third_party/ai/NOTICE.md](../third_party/ai/NOTICE.md). Source models are unmodified. Model identifiers and expected hashes remain documented in README.md.

All pipelines use one shared, telemetry-disabled ONNX environment. This is required by the pinned runtime, which allows only one environment per process even after an earlier session is dropped. Sessions retain their selected execution-provider policy.

The explicit `ai_models::tests::bundled_ai_models_acceptance` test verifies the seven installed hashes and runs synthetic CPU inference through image/text search, face detection/embedding, and OCR detection/recognition without changing its source image. CI runs it against the files extracted by the real installer, then checks update/default retention and explicit uninstall cleanup. Reports are uploaded as installed-validation artifacts. This verifies packaging and inference contracts, not model accuracy, grouping quality, GPU equivalence, or installed GUI interactions.
