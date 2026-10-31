# Development phases

## 0.56.31 Phase G scroll scheduling checkpoint

Grid and list scrolling coalesce virtual-window updates to the latest animation frame, reducing unnecessary React renders during rapid scroll input. Installed large-library acceptance remains outstanding.\n\n## 0.56.30 Phase G virtualization regression checkpoint

Both grid and list virtual windows use a common helper with 10k-row boundary tests. Installed large-library scroll and memory profiling remain outstanding.\n\n## 0.56.29 Phase G list virtualization checkpoint

List mode now renders only viewport-near rows with stable geometry, building on grid virtualization and bounded thumbnail requests. Full large-library profiling and installed acceptance remain.\n\n## 0.56.28 Phase G thumbnail queue checkpoint

Library still/video previews now use a bounded three-request queue, stale queued-item skipping, and an 80-item cache keyed by freshness and thumbnail size. HEIC retains its separate image pipeline. More performance/virtualization acceptance remains.\n\n## 0.56.27 Phase G viewport checkpoint

Library grid virtualization now measures the actual scroll viewport height instead of assuming 700px, while retaining overscan. Further Phase G work remains: thumbnail queueing, list virtualization, large-workspace profiling, and stale-work cancellation.\n\n## 0.56.26 filmstrip scheduling checkpoint

Actual timeline frames now use a shared two-request queue and a short resize/zoom debounce. Obsolete queued jobs are skipped; in-flight native extraction remains bounded and validated.\n\n## 0.56.25 adaptive filmstrip checkpoint

The timeline requests 1–8 representative frames based on clip width, including when zoom changes. Native frame work stays bounded and cached; installed Windows acceptance is still required. The 0.56.24 Rust formatting check is also corrected.\n\n## 0.56.24 timeline filmstrip checkpoint

Real timeline filmstrips sample four distinct timestamps within each trimmed video interval. Native frame extraction reuses validated bounded cached poster infrastructure; preview failures fall back to the source poster. Larger adaptive filmstrips and installed performance checks remain follow-up work.\n\n## 0.56.23 implementation checkpoint

Automatic AI indexing/counters/failures/folder policy (R2), viewer width/zoom/Live controls (R3), fullscreen library filmstrip (R4), and bounded video semantic indexing/search (F) are implemented locally and tested. Signed Preview selection/publication code (R0) is prepared, but signing/bootstrap and a genuine two-version upgrade remain unverified. This source update is pushed for manual review; no release publication is included. See [handoff](LOCAL_HANDOFF_0_56_23.md). The older 0.56.22 descriptions below are historical checkpoints.

Remaining order: installed short-media AI/viewer acceptance and signed upgrade gate; G large-library work; H maps; remaining I text/audio/transitions; J music. Vault R5/R6 remains separately gated on a security/storage design, and is not implemented. Long-form work remains in 1.x and physical-drive checks before v1.0.


Baseline: 0.56.31 source update (latest release baseline 0.56.22). This supersedes conflicting future direction in the older feedback roadmap. Keep the current working library; do not add a dashboard redesign.

## October 30 feedback — implementation checkpoint

0.56.22 implements the focused browsing/search batch: compact Library, Edits navigation, manual Sync with saved-index restoration, independent folder/collection List/Tiles preferences, relocated cleanup/manual AI tools, automatic debounced search and corrected media labels. R2 automatic indexing/counters/exclusions, R3–R4 viewers, R5–R6 vault and R0 signed Preview distribution remain planned. See the [detailed checkpoint](AI_VIEWER_VAULT_ROADMAP.md#implementation-checkpoint--05622).

Collections also gains a remembered **List / Tiles** switch, separate from Browse folders and Library. R0 adds a separate signed **Preview** update channel so future preview releases upgrade the app in place without uninstalling. The existing disabled/major-only updater may require one final signed bootstrap installation. Preserve Stable policy and app data; verify a real signed two-version upgrade before claiming the channel works. No publishing is authorized by this planning update.

Browse folders in Folders & Workspaces gains a remembered **List / Tiles** choice, with the same folder actions in either view and no automatic Sync on switching.

Latest navigation/sync decisions: add **Edits** to the left navigation as the shared editor dashboard entry. Remove Auto sync, including periodic and watcher-triggered reconciliation; use a manual **Sync** button. Restore saved library snapshots on startup. AI still automatically processes known eligible media when enabled, and new changes become eligible after explicit folder import/manual Sync. These navigation/manual-sync changes are implemented in 0.56.22.

Latest Library refinement: remove the permanent Open video editor button, search tips and Search and library tools section; place search where thumbnail zoom currently sits, and place thumbnail zoom immediately above the grid on the right. Move duplicate/similar review to Folders & Workspaces → Library cleanup. Automatic AI indexing is controlled in the AI dashboard, including per-folder **Don't index / Index folder** controls; these do not remove folders from Library. Keep the media grid high and content-focused. See R1/R2 in the linked plan for details.

Before the next feature expansion, follow [AI, viewer and vault roadmap](AI_VIEWER_VAULT_ROADMAP.md): R1 automatic search/layout/media labels; R2 automatic AI indexing with durable counters and inspectable failures; R3 details width, taller padded previews, wheel zoom and simplified Live image playback; R4 fullscreen viewer with hideable library filmstrip; R5–R6 reviewed encrypted PIN vault opened through three clicks on the version. Only the focused R1 browsing/search subset is implemented in 0.56.22; the other groups remain planned. Phase F supplies video semantic coverage to the shared indexing coordinator; keep the remaining F–J work and deferred acceptance gates.

| Phase | Deliverable | Status |
| --- | --- | --- |
| A | Design-system audit and conventions | Documented in DESIGN_SYSTEM_AUDIT.md and DESIGN.md |
| B | Hard-cut timeline: selection, reorder, trim, split, delete/ripple, playhead/zoom, durations, transactional undo/autosave | Implemented for video and supported photo clips; installed GUI acceptance remains |
| C | One workspace-owned project model/editor for one or many assets; legacy migration/recovery/relink | Shared editor, workspace storage/migration/recovery implemented in 0.56.7 |
| D | Assembled preview and validated combined MP4/H.264 export, then project-independent export presets | Continuous output/presets implemented in 0.56.8; installed GUI acceptance remains |
| E | Ranked unified discovery with match reasons and media/timestamp identity | Implemented in 0.56.11; installed GUI/AI quality acceptance remains |
| F | Bounded local video frame sampling, deduplication, embeddings, matching timestamps | Implemented in 0.56.23; installed AI/quality acceptance remains |
| G | Virtualized/lazy results, bounded thumbnail work, stale-work cancellation, scroll/memory checks | Planned |
| H | Clustered map discovery, counts/empty states, safe phone-media GPS extraction | Planned |
| I | Photos, text, local audio, simple tested transitions and social layouts after hard cuts stabilize | Photo clips implemented in 0.56.21; text/music/transitions remain |
| J | Import-your-own music first; provider-neutral source/license/attribution metadata | Planned |
| K | Private encrypted vault with PIN, version triple-click entry, lock/recovery and index isolation | Planned — R5 security/storage design precedes R6 implementation |

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

## 0.56.10 accessibility checkpoint

0.56.10 completes a small presentation/accessibility follow-up: removal actions for smart collections, manual collections, and library-folder references now use the shared destructive treatment; the details close control has a tooltip; local AI progress is announced as status. These changes do not alter media files, project state, rendering, indexing, or export behavior.

## 0.56.9 UI polish checkpoint

This patch continues the documented design-system cleanup without changing timeline storage, rendering, search indexes, or native media behavior. Remaining main destinations use the shared page-heading structure; timeline commands keep their existing semantics while gaining consistent iconography, clearer selected/empty states, and a dedicated destructive treatment for ripple deletion. Original media remains untouched when a timeline clip is removed. Deeper ranked search, video semantic indexing, large-library optimization, and map metadata work remain separate Codex/native phases.

## 0.56.11 — Phase E unified search

Reviewed the newest 0.56.10 base before implementation; its page/timeline/accessibility polish is preserved. One Library query now combines metadata and native cached evidence through a shared ranked result model. Native providers are workspace-scoped and intersect current media metadata before limits. One query vector serves all roots for an explicitly submitted meaning search. Matches retain source/type/score/reasons and the future video timestamp contract. Request ownership prevents stale results and obsolete cancellation from changing newer work.

See [UNIFIED_SEARCH.md](UNIFIED_SEARCH.md) for user instructions, scoring, limits, local-only behavior and Phase F boundaries; [CODE_REVIEW_0_56_10.md](CODE_REVIEW_0_56_10.md) records the review findings. The earlier local mixed-photo/video follow-up is preserved separately while this requested Phase E increment is completed.

Local validation before push: 123 frontend tests in 23 files passed; TypeScript and production build passed with the existing bundle-size warnings. Native formatting, warnings-as-errors lint and 140 Rust tests passed, with 6 explicit acceptance modes ignored. Tests cover provider root/model/current-media scope, read-only Unicode/URI-safe SQLite attachment, cancellation ownership, stale responses, multi-field ranking, facets, match reasons, failure fallback and sort restoration. Rendered Edge fixtures passed combined tag/OCR/meaning results, named People, favorites/ratings, provider failure, and light/dark 1220×780 / 800×580 layouts without horizontal overflow. Changed-UI detection returned no findings. Pushed GitHub Actions and installed WebView2/AI-quality acceptance remain separate gates.

## 0.56.12 — Requested navigation and viewer refinement

Restore compact sidebar rows and green section labels, add persisted menu collapse/expand with top-left workspace selection, and shorten viewer actions. This is a focused usability patch; Phase F video semantic indexing and later phases remain pending.

## 0.56.13 — Desktop editor frame

The approved Clipchamp-inspired layout is implemented over the shared editing model: project bar, searchable media, preview, side inspector and persistent timeline. Follow-up sequence: timeline filmstrips/drag trimming, photo clips, full mixed-media acceptance, then audio/text/transitions according to the existing gates. No additional provider or codec is exposed.

## 0.56.14 — Focused timeline navigation

Adaptive ruler labels, source poster thumbnails, linked-audio labels and explicit before/after reordering are implemented. Real filmstrips, drag trim handles, adjustable pane splits and photos remain follow-up work.

## 0.56.15 — Timeline trim handles

Selected-clip drag trimming and keyboard nudges use the existing rational-time trim command, shared history and revision-safe autosave. Cancelled/stale drafts never publish edits. Next remain real filmstrips, adjustable pane splits and photo clips.

## 0.56.16 — Adjustable editor split

Preview/timeline resizing now has persistent app-level sizing, pointer/keyboard interaction, cancellation and viewport-aware limits. Project history and rendering are unchanged. Filmstrips and photo clips remain next.

## 0.56.20 — CI alignment

Remaining frontend tests were migrated from the removed numeric/player trim UI to the timeline-handle workflow introduced in 0.56.18. Runtime behavior is unchanged.

## 0.56.19 — Playback and appearance preferences

Video playback volume is now a persisted global preference shared by normal video playback and assembled project preview. Library video tiles and Live Photos gain lazy muted hover motion previews. Settings adds five stock accent palettes (Purple, Blue, Teal, Green, Rose) that reuse the existing semantic design tokens in light and dark themes. These changes are presentation/playback preferences only and do not alter project or export semantics.

## 0.56.18 — Timeline-first trimming and preview resizing

Editor trimming is now timeline-only: the selected clip's left/right handles are the source-of-truth UI and duplicate numeric/source-player trim controls are removed. The preview/timeline divider stays live during layout measurement, the preview resizes with the divider, and timeline zoom uses direct out/fit/in controls. Timeline clips show a filmstrip-style repeated poster using the existing bounded poster cache; true sampled frames remain a future native enhancement. Crop controls are surfaced by default in the Clip inspector.

## 0.56.17 — Inspector task clarity

Clip editing and project canvas/export settings now have separate accessible tabs with direct Canvas access. Hidden panels retain drafts, and output validation reveals invalid clip fields. Filmstrips and photos remain follow-ups.


## 0.56.21 — Shared photo/video timeline

The existing typed-media work is now integrated with the newer timeline-only trimming, inspector tabs, native playback controls, persisted volume and accent themes. Photos have geometry and an explicit virtual duration, rather than fabricated video stream metadata. Schema 3 migrates v1/v2 video documents at the native boundary. Photos use the existing clip commands and durable session; no second editor was introduced. Continuous rendering normalizes one owned still at a time, with the same geometry filters, cumulative frame clock, cancellation, source verification and safe publication as video.

Local short-fixture coverage checks mixed and photo-only decode, frame allocation, orientation, alpha flattening, silent photo intervals, persistence/recovery/relink and source preservation. Browser fixtures cover duration/undo, original photo display, assembled playback and light/dark/minimum-window layout. Installed Windows interaction/listening acceptance remains separate. Next planned core phase is F (bounded local video semantic indexing); remaining I/J extensions follow in small patches. Long-form work stays deferred to 1.x, and physical-drive failure checks remain a pre-v1.0 gate.
