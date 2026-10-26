# Open-Photo

**Open-Photo is a privacy-first Windows photo organizer for local photo and video libraries.**

It is designed to work directly with folders you already own, keep original media read-only, store rebuildable indexes locally, and make optional AI features run on the computer instead of sending photos to a cloud recognition service.

> **Development status:** Open-Photo is actively developed as a 0.x project. Many library-management features are working today, while People/face grouping, OCR, GPU validation, full RAW support, and Immich synchronization are still unfinished. Do not use Open-Photo as the only copy of important photos or metadata.

## What works today

| Area | Status | Notes |
| --- | --- | --- |
| Windows desktop app | ✅ Working | Tauri 2 + React/TypeScript + Rust |
| Local folder scanning | ✅ Working | Recursive, read-only source scan with path validation |
| Multiple workspace folders | ✅ Working | One workspace can include multiple local roots |
| Persistent media index | ✅ Working | SQLite index under local app data |
| Incremental folder updates | ✅ Working | Native watcher plus safe full-scan fallback |
| Manual and periodic sync | ✅ Working | Manual Sync Now and optional 2-minute auto-sync |
| Grid/list library views | ✅ Working | Filename search, media filters, sorting, thumbnail-size control |
| Local thumbnail cache | ✅ Working | Bounded disk cache; originals are not changed |
| Ratings and favorites | ✅ Working | 1–5 stars stored as local Open-Photo state; 5 stars acts as Favorite |
| Virtual collections | ✅ Working | Workspace-scoped collections without moving files |
| Smart collections | ✅ Working | Rating, size, media type, recent media, memories, unrated and similar presets |
| Timeline | ✅ Working with limitation | Uses filesystem modified date, not full EXIF/QuickTime capture date yet |
| GPS map | ✅ Working with privacy note | Reads EXIF GPS locally; map tiles are loaded from OpenStreetMap |
| Full-screen viewer | ✅ Working | Keyboard navigation and larger local preview |
| XMP keyword sidecars | ✅ Working, intentionally limited | Creates new sidecars only; never overwrites an existing sidecar |
| Exact duplicate detection | ✅ Working | SHA-256 based review across workspace roots |
| Visual near-duplicate review | ✅ Working | Local CPU dHash grouping; review only, no automatic deletion |
| Preferred duplicate copy | ✅ Working | User choice is stored locally and does not move/delete files |
| iPhone HEIC/HEIF previews | ✅ Partial | Decoded locally; see iPhone notes below |
| Live Photo pairing | ✅ Partial | Same folder + filename stem heuristic |
| Local ONNX inference | ✅ Working foundation | CPU-first ONNX Runtime integration |
| AI model import/removal | ✅ Working | Manual, local, checksum-verified and confined to app data |
| Persistent semantic image embeddings | ✅ Working | Stored locally in SQLite and model/pipeline-versioned |
| Incremental AI indexing | ✅ Working foundation | Selected or full-library indexing; already-current embeddings are skipped |
| AI photo-to-photo similarity | ✅ Working | Cosine similarity over locally stored embeddings |
| Natural-language semantic search | 🧪 Experimental | 0.25.x code path; requires the exact supported OpenCLIP bundle and further end-to-end runtime validation |
| Face recognition / People | 🧪 Experimental | YuNet detection + SFace embeddings + local suggested groups; manual naming is local-only |
| OCR | 🧪 Experimental | English-first PP-OCR detector + recognizer, local text index and photo-text search |
| GPU acceleration | 🧪 Windows foundation | Automatic mode prefers DirectML when available and safely falls back to CPU |
| Full RAW preview pipeline | 🚧 Not implemented | RAW files can be indexed but may not preview |
| Immich synchronization | 🚧 Not implemented | Architecture planning only; no API bridge is active |
| Non-destructive photo editing/export | 🧪 Experimental | Rotate, flip, brightness and contrast; always saves a new copy and never overwrites originals |
| Original photo deletion | 🚫 Not implemented | Open-Photo does not delete original media |

See [CHANGELOG.md](CHANGELOG.md) for the version history.

## Core safety model

Open-Photo treats your source media as protected content.

- Original photos and videos are scanned and previewed read-only.
- File paths are canonicalized before media commands are allowed to read them.
- Commands reject media outside the selected library root.
- Symlink escapes are not followed during normal scanning.
- Deletion and original-media editing are not implemented.
- XMP keyword writing is explicit and creates a **new missing sidecar only**. Existing XMP sidecars are not overwritten.
- AI models, indexes, embeddings, ratings and other application state live outside the original photo folders unless you explicitly create an XMP sidecar.
- A truncated or inaccessible scan is handled conservatively so an unavailable drive is not treated as a reason to purge source media.

## Getting started as a tester

Open-Photo does not yet have a formal public release channel. Windows installers are produced by the **Windows Desktop Build** GitHub Actions workflow after formatting, frontend build, Rust lint, regression tests and the Tauri installer build all pass.

For a test build:

1. Open the latest **green** Windows Desktop Build workflow.
2. Download the generated Windows artifact.
3. Extract the artifact and run the NSIS installer.
4. Treat 0.x builds as development software and keep normal backups of your photo library.

A red workflow means that development version should **not** be treated as the current verified build.

## Basic library workflow

### 1. Create or open a workspace

Use **Open Folder** or **Add folder** from the Library view.

A workspace can contain more than one local folder. Open-Photo records the folder references; it does not import or relocate the files.

### 2. Sync the library

Use **Sync Now** to refresh the local SQLite index.

You can also enable **Auto-sync every 2 minutes**. Open-Photo additionally listens for native filesystem events and applies bounded incremental updates when possible. If an event cannot be handled safely, it falls back to a normal indexed scan.

The sync summary reports added, updated, unchanged and removed index entries. Removing an item from the index is not the same as deleting a source file.

### 3. Browse and filter

The Library view supports:

- Grid and list views
- Filename search
- Photo / video filtering
- Newest, oldest, name, largest and smallest sorting
- Adjustable thumbnail size
- Folder filters
- Favorites and rated-photo filters
- Smart collection filters

Select a photo to open the details viewer. Use the full-screen control for a larger preview.

### 4. Ratings and favorites

Open a photo and choose 1–5 stars.

Ratings are Open-Photo application state and currently do **not** write into the original file or an XMP sidecar. A 5-star rating is treated as a Favorite by the current Favorites filter.

### 5. Virtual collections

Virtual collections group existing file paths without moving or copying source media.

Collections are workspace-scoped. Workspace/collection metadata can be exported with **Export Workspace JSON** from Settings.

### 6. Smart collections

Open-Photo includes dynamic filters such as:

- Last 30 days
- On This Day
- This month in previous years
- Large media
- Unrated media

You can also create workspace-scoped smart rules based on rating, media type and minimum file size.

Current date-based smart filters use filesystem modification timestamps. Full capture-date indexing from EXIF and QuickTime metadata is still planned.

## Edit & Export

The first Phase 4 editor is intentionally non-destructive.

Open a supported photo and use **Edit & Export** to apply:

- 90° rotation
- Horizontal flip
- Vertical flip
- Brightness from -100 to +100
- Contrast from -100 to +100

Choose **Save edited copy** and select a new destination filename.

Safety behavior:

- The source photo is opened read-only.
- The export command rejects a destination that resolves to the original source.
- Existing destination files are never overwritten; choose a new filename instead.
- If encoding or final disk sync fails, the partial output is removed.
- Export is currently limited to JPG/JPEG, PNG, WebP, BMP and TIFF.
- This first editor pass supports locally decodable JPG/JPEG, PNG, WebP, GIF, TIFF, BMP and AVIF sources.
- HEIC/HEIF, RAW and video editing are intentionally excluded until dedicated decode/export handling is validated.
- Metadata preservation is not yet guaranteed for edited exports; treat an edited copy as a newly rendered image.

## XMP keyword sidecars

Open-Photo can create keyword sidecars for selected media.

1. Select one or more files.
2. Enter comma-separated tags in the selection toolbar.
3. Choose **Add tags to new sidecars**.

Important behavior:

- Maximum batch: 1,000 selected files.
- Maximum tags: 50.
- Maximum tag length: 128 characters.
- The sidecar is created next to the source using the source filename plus .xmp.
- Existing sidecars are skipped rather than overwritten.
- The original photo/video is not edited.
- This is currently a create-only keyword workflow. Hierarchical tag merging, captions, ratings, face regions, sidecar editing and undo are not complete.

## Duplicate tools

Open-Photo has two different duplicate systems.

### Exact duplicates

**Find Exact Duplicates** compares local media using SHA-256-backed indexed duplicate detection. It can inspect media across workspace roots.

### Visually similar photos

**Find Similar Photos** uses a local CPU dHash comparison to group likely near-duplicates.

Both tools are review-only:

- Preview each candidate.
- Optionally mark a preferred copy.
- Open-Photo does not automatically delete, move or replace anything.
- The preferred-copy choice is local application state, not embedded metadata.

The newer AI semantic similarity feature is separate from dHash and requires AI embeddings.

## Map and location privacy

The Map view reads GPS metadata from supported photos and displays those points on a Leaflet map.

**Privacy note:** photo files and EXIF data are processed locally, but opening the Map view requests map tiles from **OpenStreetMap** at https://tile.openstreetmap.org. The requested tile coordinates necessarily correspond to the visible map area. If you require a fully offline workflow, avoid opening the Map view until offline tiles are supported.

## iPhone media notes

Open-Photo has partial iPhone compatibility.

### HEIC / HEIF

HEIC/HEIF images can be read from the selected library and decoded locally for preview.

### Live Photos

Open-Photo currently pairs a Live Photo image and motion clip when they are:

- in the same folder,
- share the same filename stem, and
- form an unambiguous image/video pair.

This is a filename-based heuristic, not full Apple asset-identifier matching.

### Motion video

MOV/MP4/M4V motion can be previewed inline when the Windows/WebView2 codec stack supports it. A fallback action opens the video in the default Windows media player.

Special limitations:

- Inline iPhone media reads are bounded; very large motion clips may need the external player.
- HEVC playback depends on Windows/WebView2 codec availability.
- HDR/color behavior has not been exhaustively validated.
- RAW + Live Photo combinations are not a finished workflow.

## Local AI

Local AI is **opt-in and off by default**.

Open **Settings → Local AI** and enable **Allow local AI features when available**.

Current AI architecture:

- ONNX Runtime executes models locally.
- CPU remains the compatibility fallback. On Windows, Automatic mode now attempts the ONNX Runtime DirectML execution provider first and falls back to CPU if DirectML cannot be registered.
- Models are manually imported; Open-Photo does not silently download them.
- Settings exposes backend-derived compatibility status for every pinned AI slot and shows whether Semantic Search, People and OCR are ready.
- **Run AI self-check** reports bundle readiness, DirectML availability, CPU fallback and whether real-world validation is still outstanding.
- Imported assets are copied into Open-Photo's private local app-data model directory.
- SHA-256 verification is required.
- Semantic model slots reject assets that do not match the currently supported OpenCLIP bundle.
- AI actions are disabled until their complete pinned model bundle is installed and verified.
- People model slots reject anything except the pinned YuNet 2026may detector and SFace 2021dec embedder.
- Image embeddings are stored in a separate local SQLite database.
- Original photos are never modified by AI indexing.

### Supported OCR model bundle

The first OCR milestone is intentionally **English-first** and uses RapidOCR/PaddleOCR-compatible mobile ONNX models:

| Open-Photo slot | Upstream model | Required SHA-256 |
| --- | --- | --- |
| ocr | en_PP-OCRv3_det_mobile.onnx | ea07c15d38ac40cd69da3c493444ec75b44ff23840553ff8ba102c1219ed39c2 |
| ocr_recognizer | en_PP-OCRv4_rec_mobile.onnx | e8770c967605983d1570cdf5352041dfb68fa0c21664f49f47b155abd3e0e318 |

Open-Photo validates both assets before inference and supplies their pinned hashes automatically during import.

Current OCR behavior:
- Runs locally on CPU or DirectML using the existing AI processing preference.
- Detects text regions from a PP-OCR probability map.
- Uses conservative rectangular connected components for the first implementation rather than permanently storing unverified polygons.
- Recognizes text with a fixed 48×320 English PP-OCR input pipeline and CTC decoding.
- Stores extracted text and region metadata in `ai-ocr.sqlite` under local app data.
- Skips already-current photos when the source timestamp and model pair are unchanged.
- Lets Library users index the current workspace for OCR and search indexed photo text locally.
- Never writes OCR text into original photos or XMP automatically.

This first OCR pass is experimental. Curved text, rotated text, dense documents, handwriting and non-English languages need later model/post-processing work.

### Supported People model bundle

People indexing uses two pinned OpenCV Zoo models:

| Open-Photo slot | Upstream file | Required SHA-256 | License |
| --- | --- | --- | --- |
| faces | face_detection_yunet_2026may.onnx | ebafce4e3c118d6554634be5c27ab333b4c047a9a8c3faf1d7cf93101c22f0f0 | MIT |
| face_embedder | face_recognition_sface_2021dec.onnx | 0ba9fbfa01b5270c96627c4ef784da859931e02f04419c829e83484087c34e79 | Apache-2.0 |

Download the exact upstream files yourself, then import them in Settings. Open-Photo supplies the pinned checksum automatically for these known slots and refuses a mismatched model.

The current pipeline:
- resizes and pads photos locally for YuNet detection;
- decodes bounding boxes and five facial landmarks locally;
- aligns faces to the standard 112×112 ArcFace five-landmark template;
- generates normalized 128-value SFace embeddings;
- stores boxes, embeddings and model provenance in `ai-faces.sqlite` under local app data;
- skips photos that are already current for the same source modification time and model pair;
- creates local suggested People groups by embedding similarity;
- lets the user apply local display names to suggested groups.

Suggested groups are intentionally advisory. Open-Photo does not write face regions or names into original media or XMP yet, and it does not automatically merge identities permanently.

### Supported semantic model bundle

The current semantic pipeline is explicitly built around:

**OpenCLIP ViT-B/32 — LAION-2B, split ONNX export**  
Source: https://huggingface.co/amikos/openclip-vit-b-32-laion2b-s34b-b79k-onnx

Import these exact files in Settings:

| Open-Photo slot | Upstream file | Required SHA-256 |
| --- | --- | --- |
| semantic | vision_model.onnx | 7e14f76233d0c840c0621b1ef68f5877efe9357850782b1bbaf0c01693f73b43 |
| semantic_text | text_model.onnx | 252b86e0ef1fc95b22cfd52fbf647142727fdbecc152556ffe0fba0b10a80370 |
| semantic_tokenizer | tokenizer.json | b556ac8c99757ffb677208af34bc8c6721572114111a6e0aaf5fa69ff0b8d842 |

The upstream bundle is roughly 600 MB combined. Consult its model card and license before redistribution or deployment. Semantic search should be considered experimental until it has been tested against representative real-world photo libraries.

### Importing the semantic bundle

For each slot:

1. Download the matching upstream file yourself.
2. In Settings, choose the import action for that slot.
3. Select the downloaded file.
4. Paste the trusted SHA-256 value when prompted.
5. Open-Photo verifies the checksum before copying the asset.

An existing slot is never silently overwritten. Remove the installed slot explicitly before replacing it.

### Index photos with AI

For one photo:

1. Open the photo details panel.
2. Choose **Index this photo**.

For multiple photos:

1. Select files in the Library.
2. Choose **Index selected with AI**.

For the current workspace:

1. Choose **Index library with AI**.
2. Open-Photo groups photos by workspace root and indexes each root locally.
3. Files whose modification time, model/preprocessing key and 512-value embedding are already current are skipped instead of recomputed.

The backend accepts up to 5,000 photos per batch, and the Library UI chunks larger root groups automatically. One ONNX vision session is reused for each batch rather than reloading the model for every image. The heavy batch command runs off the main UI thread.

The image model creates a normalized 512-value embedding for each supported image. Embeddings are stored locally and are tied to the supported model/preprocessing pipeline so incompatible or older embeddings can be detected as stale.

Current AI-decodable library indexing is limited to JPG/JPEG, PNG, WebP, GIF, TIFF, BMP and AVIF. HEIC/HEIF, RAW formats and video can still exist in the library, but the semantic image pipeline does not currently index them.

Multi-folder workspace semantic text search now queries each workspace root and merges the best local matches. This still needs representative end-to-end runtime testing with the verified model bundle.

### Find AI-similar photos

After photos have current embeddings:

1. Open an indexed photo.
2. Choose **Find AI-similar photos**.
3. Open-Photo compares local 512-value embeddings with cosine similarity.
4. Results are ranked locally.

This is different from the non-AI dHash similar-photo tool.

### Natural-language semantic search

The 0.25.x development line introduced the text-query path, with 0.26.x adding incremental library-scale indexing and multi-root search orchestration. Example queries include:

- dog at the beach
- sunset over water
- birthday cake
- people hiking in the woods

The text encoder and image encoder are validated as one compatible OpenCLIP bundle and produce aligned 512-value embeddings.

**Status:** experimental. The code path still requires a green build and end-to-end testing with the verified bundle before it should be considered a finished user feature.

If the image preprocessing/model contract changes, re-index the affected photos rather than mixing old and new embeddings.

## Local data locations

On Windows, Open-Photo uses the user's local application-data/cache folders.

Typical local application data under %LOCALAPPDATA%\Open-Photo includes:

- media-index.sqlite — rebuildable media index
- ai-embeddings.sqlite — local semantic embeddings
- ai-models\ — imported local AI assets

Preview thumbnails use the Windows cache directory under an Open-Photo\previews-v1 folder and are bounded/pruned.

Browser-side preferences such as ratings, collections, smart rules, theme and some workspace settings are currently stored in local application storage.

Deleting Open-Photo application data can remove indexes, embeddings and local-only organization state even though it does not delete the original photos. Export workspace metadata and keep backups of anything important.

## Supported media

The scanner currently recognizes common desktop, iPhone, RAW and video extensions including:

**Images:** JPG/JPEG, PNG, WebP, GIF, TIFF, BMP, HEIC/HEIF, AVIF, DNG, CR2, NEF, ARW  
**Video:** MOV, MP4, M4V, AVI, MKV, WebM

Recognition by the scanner does not guarantee that every format can be decoded for thumbnails, AI indexing or inline playback. RAW preview support is especially incomplete.

## Library health check

Settings includes a read-only **Library health check**.

It reports indexed, available, missing, inaccessible and modified items without deleting files or index records.

If the selected root itself is offline/unavailable, the command can return an unavailable-root error rather than treating the entire library as deleted. This is intentional safety behavior.

## Features still in development

The following should not be described as finished:

- Face detection and recognition
- People grouping, person merge/review and portable face regions
- OCR and text-in-image search
- GPU / DirectML / CUDA acceleration
- Background AI queue with pause/resume/progress and smart incremental scheduling
- Automatic AI model download/catalog management
- Full EXIF/QuickTime capture-date timeline
- Full RAW rendering and RAW-specific color handling
- Complete hierarchical XMP editing/merging
- Non-destructive photo editing and export presets
- Safe deletion/trash workflows
- Fully offline map tiles
- Metadata-based Apple Live Photo asset pairing
- Immich API synchronization and person mapping

## Development setup

Open-Photo is built with Tauri 2, React 19, TypeScript, Rust and SQLite.

Recommended Windows development environment:

- Windows 10/11
- Node.js 22 (matches CI)
- Rust stable with rustfmt and Clippy
- Tauri 2 Windows prerequisites, including the Microsoft C++ build tools and WebView2 runtime

Clone and install:

~~~powershell
git clone https://github.com/MieleMadness/Open-Photo.git
cd Open-Photo
npm install
npm run tauri dev
~~~

Useful validation commands:

~~~powershell
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri build
~~~

The Windows CI runs these gates before publishing the installer artifact.

## Version policy

During 0.x development:

- If the previous build is **green**, feature work advances the **minor** version.
- If the previous build is **red**, the repair advances the **patch** version.
- A major release is reserved for an explicitly approved main release.

The version must match in package.json, src-tauri/Cargo.toml and src-tauri/tauri.conf.json.

## Privacy and network behavior

Open-Photo does not require an account.

Local library scanning, thumbnails, indexes, ratings, duplicate checks and AI inference are designed to stay on the computer.

Known network-sensitive behavior:

- The Map view loads OpenStreetMap tiles when opened.
- AI model files are downloaded manually by the user from their chosen source; Open-Photo does not currently fetch them itself.
- Future Immich integration is not implemented.

## Repository and licensing status

The GitHub repository is currently **private** and GitHub reports **no repository license**. The project is being developed toward an open-source release, but public reuse rights should not be assumed until a license file is deliberately added.

Third-party dependencies and AI model assets retain their own licenses and usage terms.

## Additional documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Historical milestone plan](docs/MILESTONES.md)
- [Contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)
