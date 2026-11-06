# Open-Photo product context

Confirmed by the owner's development brief, 29 October 2026.

- Tauri + React, offline-first Windows desktop photo/video organizer and basic editor.
- Users manage local folders, photos, videos, people, tags, indexes, collections, and editing projects. The owner is not a developer; instructions and recovery must use plain language.
- A workspace is the library/environment boundary and contains many projects. A project is a created work inside that workspace, not a workspace itself.
- One video and several videos use the same project model, state management, and editor. Begin with a single sequential hard-cut timeline and linked original audio.
- Preserve existing photo, video, AI, indexing, and export functionality. Original media is never destructively modified by default; edited/exported outputs are separate, safely published files.
- Keep AI/search local. Sample video frames selectively instead of indexing every frame. Optional external services must not become required dependencies.
- The design is a calm, professional native desktop utility: content first, efficient density, strong themes, accessible controls, and subtle purposeful motion. The existing visual identity is retained.
- Use short test clips. Further long-form work is deferred to 1.x; physical drive-failure acceptance remains a pre-v1.0 gate.
- Ship small reviewable patches with synchronized versions/documentation, frontend and native checks, and verified GitHub Actions results. No speculative codec/container options or untested transitions.

## Platform and stack

Windows desktop with a React/TypeScript WebView frontend and Rust/Tauri backend. Existing library and media safety contracts are authoritative. The UI uses desktop conventions; mobile-specific assumptions do not override them.

## Implementation order

Design-system audit; hard-cut timeline; unified single/multi editor; continuous combined export; export presets; ranked unified search; sampled video semantic indexing; large-library performance; map discovery; social editor extensions; local music architecture.
