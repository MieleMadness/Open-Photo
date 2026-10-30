# Development phases

Baseline: 0.56.10. This supersedes conflicting future direction in the older feedback roadmap. Keep the current working library; do not add a dashboard redesign.

| Phase | Deliverable | Status |
| --- | --- | --- |
| A | Design-system audit and conventions | Documented in DESIGN_SYSTEM_AUDIT.md and DESIGN.md |
| B | Hard-cut timeline: selection, reorder, trim, split, delete/ripple, playhead/zoom, durations, transactional undo/autosave | Implemented for video clips; installed GUI acceptance remains |
| C | One workspace-owned project model/editor for one or many assets; legacy migration/recovery/relink | Shared editor, workspace storage/migration/recovery implemented in 0.56.7 |
| D | Assembled preview and validated combined MP4/H.264 export, then project-independent export presets | Continuous output/presets implemented in 0.56.8; installed GUI acceptance remains |
| E | Ranked unified discovery with match reasons and media/timestamp identity | Planned — Codex/native indexing work |
| F | Bounded local video frame sampling, deduplication, embeddings, matching timestamps | Planned — Codex/native AI work |
| G | Virtualized/lazy results, bounded thumbnail work, stale-work cancellation, scroll/memory checks | Planned |
| H | Clustered map discovery, counts/empty states, safe phone-media GPS extraction | Planned |
| I | Photos, text, local audio, simple tested transitions and social layouts after hard cuts stabilize | Gated on B–D |
| J | Import-your-own music first; provider-neutral source/license/attribution metadata | Planned |

## Architecture decisions

Timeline edits operate on ordered clip instances and rational source times. Their commands do not depend on where files or drafts are stored. A split/reorder/trim/delete is one undoable project transaction; original audio stays with its clip.

Projects are owned by the workspace. Local verified bindings resolve each asset within that workspace's registered roots; portable documents carry relative references and fingerprints. Legacy drafts are copied into the shared model, without trusting imported absolute paths or keeping a second editor.

The initial track is gapless: Delete and Ripple delete both close the removed interval. A future gap/track representation needs a versioned migration and tested silence/black semantics. Stable asset/clip identities and separate source versus timeline time leave room for photos, overlays, transitions, and additional audio tracks.

Preview and export must compile from the same sequence. A playlist of independent HTML players is not evidence of continuous output. Presets resolve into an output specification separately from edit decisions. Continue using staged, validated, no-overwrite publication and source revalidation.

## Delivery gate

Every pushed functional patch increments the patch version and updates the npm/Cargo/Tauri versions, changelog, README, milestone/roadmap and relevant user instructions. Run frontend tests, TypeScript/build, native formatting/lint/tests, and inspect the resulting GitHub workflow. Use short fixtures; keep long-form and physical-drive failure gates explicitly deferred.

## 0.56.6 implementation checkpoint

The first timeline increment implements ordered clips, rational trims/splits, drag and button reorder, ripple deletion, repeated instances, playhead/zoom/durations, undo/redo, empty-timeline persistence, and revision-safe autosave. This is a partial B milestone. The player remains an explicitly labeled original-source preview. Next: workspace-owned persistence and the shared editor, then one compiled sequence for preview and export. Photo clips join that editor after continuous hard-cut output is stable.

The workspace dropdown belongs at the top left. There is no separate Video Projects navigation tab. Library selection should create one continuous editing project; one clip and many clips ultimately enter the same workflow.

Validation for 0.56.6 before push: 97 frontend tests passed; TypeScript and production build passed (existing bundle-size warnings remain); native formatting, warnings-as-errors lint, and 115 Rust tests passed, with 6 explicitly ignored acceptance modes. Rendered fixtures passed light/dark and 1220×780 / 800×580 checks, actual drag reorder, button reorder, undo, and persistent top-left workspace geometry. The timeline detector returned no findings. These checks do not claim assembled preview/export or mixed-photo playback. GitHub Actions must be checked against the pushed commit separately.

## 0.56.7 implementation checkpoint

Project ownership is now the workspace ID. The schema-v2 portable edit document is retained, while each asset’s verified source-folder binding is local metadata. Keeping ownership and local path resolution outside the portable document lets a project move without granting arbitrary file access. Native project actions read the registered roots from durable workspace settings. Missing roots affect source readiness, not project identity or edit availability.

Legacy v1/v2 drafts are copied once in one transaction, with migration provenance. Old documents and revisions remain unchanged. Unreadable drafts produce warnings rather than hiding valid projects. Autosave updates use optimistic revisions and backup the prior document/bindings together. Recovery creates a new project, keeps the current draft, and checks workspace/project/history/revision identity. The old single-video controller is removed. Both entry paths now use the same timeline, session, selected-clip inspector and output controls. One-clip preview/export uses an internal compatibility adapter to the proven rendering pipeline, reading the saved workspace revision and authorized source binding. This adapter stores no second draft and owns no editor state. Continuous multi-clip rendering is the next milestone.

Legacy migration also copies up to 20 previous saves per draft. Older revisions with no recorded date or revision keep that information unavailable; no historical dates are invented. Player trim markers update the selected timeline instance, and source-changing operations lock those markers until verification finishes. Reset selected clip restores its source interval/edits; Reset project canvas separately restores the default canvas.

Validation for 0.56.7 before push: 103 frontend tests passed; TypeScript and production build passed with the existing bundle-size warnings; native formatting, warnings-as-errors lint, and 122 Rust tests passed, with 6 explicitly ignored acceptance modes. Native cases include cross-folder short-video import/dedup/rollback, source fingerprints and workspace boundaries, legacy draft/history migration, revision-safe recovery, and resuming/relinking a matching one-source project. Browser fixtures passed shared-editor entry, multi-clip editing, recovery disclosure, light/dark, normal/minimum window geometry, drag/button reorder, and undo. The changed-UI detector returned no findings. GitHub Actions must be verified for this pushed commit separately. Multi-clip output remains gated.

## 0.56.8 implementation checkpoint

Continuous hard-cut preview/export and independent resolution presets are implemented. Both compile one immutable saved sequence, reusing source validation and edit preparation. Each source is decoded in turn into owned temporary parts; final video is stream-copied and audio is encoded once across the sequence. Cumulative frame allocation keeps timeline, preview and export aligned. The stage has one player owner and releases detached decoders. Current verified relinks survive recovery of older edits for the same original.

Short native fixtures cover mixed frame rates/shapes, order/trim, repeated assets, gain/mute/silence, delayed/multiple audio, preview/export agreement, landscape/portrait/square/4K, cancellation, no-space, changed-source and destination-race failures. No long-form footage is used. Photos, text/music/transitions remain in I, after E ranked discovery, F video indexing, G large-library work and H map improvements. Packaged GUI/hardware acceptance and the deferred physical-drive checks remain distinct from these automated checks.

Validation before push: 111 frontend tests in 21 files; TypeScript and production build; native formatting, warnings-as-errors lint, and 131 Rust tests (6 explicit acceptance modes ignored). Existing build chunk-size warnings remain. Real native-produced H.264/AAC preview played and sought in Edge through the assembled stage, with one video element, source restoration/cancellation, light/dark and 1220×780 / 800×580 no-overflow checks. Rendered review also corrected sub-second timeline sizing; StrictMode regression coverage protects ref reattachment from decoder cleanup. The changed-UI detector returned no findings. Pushed GitHub Actions must be verified for the exact commit separately.

## 0.56.10 accessibility checkpoint\n\n0.56.10 completes a small presentation/accessibility follow-up: removal actions for smart collections, manual collections, and library-folder references now use the shared destructive treatment; the details close control has a tooltip; local AI progress is announced as status. These changes do not alter media files, project state, rendering, indexing, or export behavior.\n\n## 0.56.9 UI polish checkpoint

This patch continues the documented design-system cleanup without changing timeline storage, rendering, search indexes, or native media behavior. Remaining main destinations use the shared page-heading structure; timeline commands keep their existing semantics while gaining consistent iconography, clearer selected/empty states, and a dedicated destructive treatment for ripple deletion. Original media remains untouched when a timeline clip is removed. Deeper ranked search, video semantic indexing, large-library optimization, and map metadata work remain separate Codex/native phases.
