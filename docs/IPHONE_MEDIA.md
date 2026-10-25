# iPhone media compatibility (Windows)

Open-Photo indexes HEIC, HEIF, JPEG, MOV, MP4 and M4V originals without editing or deleting them.

## HEIC/HEIF photos
The Windows app decodes eligible HEIC/HEIF files in the local WebView using **heic-to**, backed by libheif. Decoded JPEGs exist only as temporary previews; source files are never overwritten. A 48 MiB read limit protects the preview process. Color/HDR and portrait-depth rendering vary with the decoder; representative iPhone samples still require manual validation.

## Live Photos
For now, **one photo and one MOV in the same folder sharing an exact filename stem** are presented as one still-photo library item with a motion control. The video remains visible in the Video filter. The pair is only a **probable match**: Apple asset-identifier metadata is not yet parsed, and renamed or differently exported pairs may not match. We deliberately avoid grouping ambiguous stems.

## Video and motion
The app attempts in-view video playback using Windows WebView2. Supported codecs depend on Windows and WebView2. Some iPhone MOV/HEVC/HDR videos will not render in the embedded player. The app retains an **Open in Windows player** fallback, and files over 48 MiB use that fallback. Reliable HEVC transcoding and broader HDR color handling are follow-up work.

## Third-party codec licensing
**heic-to** (including its bundled libheif decoder) is distributed under **LGPL-3.0**. Its source and license are available at https://github.com/hoppergee/heic-to and https://github.com/strukturag/libheif . Distribution must preserve required notices and comply with LGPL obligations. The dependency is dynamically imported by the frontend when needed. Review license compliance before distributing installer binaries publicly.

## Validation
Use iPhone sample libraries covering HEIC, HEIF, HDR, Portrait, JPEG, ordinary MOV/MP4, Live Photos, HEVC, duplicate stems, renamed pairs, disconnected drives and files above the inline limit. A successful compilation is not proof that all codecs play on every Windows system.
