# Development phases

Baseline: 0.56.5. This supersedes conflicting future direction in the older feedback roadmap. Keep the current working library; do not add a dashboard redesign.

| Phase | Deliverable | Status |
| --- | --- | --- |
| A | Design-system audit and conventions | Documented in DESIGN_SYSTEM_AUDIT.md and DESIGN.md |
| B | Hard-cut timeline: selection, reorder, trim, split, delete/ripple, playhead/zoom, durations, transactional undo/autosave | In progress |
| C | One workspace-owned project model/editor for one or many assets; legacy migration/recovery/relink | Next |
| D | Assembled preview and validated combined MP4/H.264 export, then project-independent export presets | Planned |
| E | Ranked unified discovery with match reasons and media/timestamp identity | Planned |
| F | Bounded local video frame sampling, deduplication, embeddings, matching timestamps | Planned |
| G | Virtualized/lazy results, bounded thumbnail work, stale-work cancellation, scroll/memory checks | Planned |
| H | Clustered map discovery, counts/empty states, safe phone-media GPS extraction | Planned |
| I | Photos, text, local audio, simple tested transitions and social layouts after hard cuts stabilize | Gated on B–D |
| J | Import-your-own music first; provider-neutral source/license/attribution metadata | Planned |

## Architecture decisions

Timeline edits operate on ordered clip instances and rational source times. Their commands do not depend on where files or drafts are stored. A split/reorder/trim/delete is one undoable project transaction; original audio stays with its clip.

The current schema-v2 store is keyed by a source folder. That is an existing implementation limitation, not the intended product model. Phase C must move project ownership to a workspace and resolve each asset only within that workspace's selected roots. Keep legacy drafts unchanged and migrate into the shared model; do not create a permanent single-video editor or trust imported absolute paths.

The initial track is gapless: Delete and Ripple delete both close the removed interval. A future gap/track representation needs a versioned migration and tested silence/black semantics. Stable asset/clip identities and separate source versus timeline time leave room for photos, overlays, transitions, and additional audio tracks.

Preview and export must compile from the same sequence. A playlist of independent HTML players is not evidence of continuous output. Presets resolve into an output specification separately from edit decisions. Continue using staged, validated, no-overwrite publication and source revalidation.

## Delivery gate

Every pushed functional patch increments the patch version and updates the npm/Cargo/Tauri versions, changelog, README, milestone/roadmap and relevant user instructions. Run frontend tests, TypeScript/build, native formatting/lint/tests, and inspect the resulting GitHub workflow. Use short fixtures; keep long-form and physical-drive failure gates explicitly deferred.

## 0.56.6 implementation checkpoint

The first timeline increment implements ordered clips, rational trims/splits, drag and button reorder, ripple deletion, repeated instances, playhead/zoom/durations, undo/redo, empty-timeline persistence, and revision-safe autosave. This is a partial B milestone. The player remains an explicitly labeled original-source preview. Next: workspace-owned persistence and the shared editor, then one compiled sequence for preview and export. Photo clips join that editor after continuous hard-cut output is stable.

The workspace dropdown belongs at the top left. There is no separate Video Projects navigation tab. Library selection should create one continuous editing project; one clip and many clips ultimately enter the same workflow.

Validation for 0.56.6 before push: 97 frontend tests passed; TypeScript and production build passed (existing bundle-size warnings remain); native formatting, warnings-as-errors lint, and 115 Rust tests passed, with 6 explicitly ignored acceptance modes. Rendered fixtures passed light/dark and 1220×780 / 800×580 checks, actual drag reorder, button reorder, undo, and persistent top-left workspace geometry. The timeline detector returned no findings. These checks do not claim assembled preview/export or mixed-photo playback. GitHub Actions must be checked against the pushed commit separately.
