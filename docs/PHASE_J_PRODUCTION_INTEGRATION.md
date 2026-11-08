## Separated local review checkpoint — 2026-11-08

This 0.56.88 checkpoint contains only the local Phase J production integration on parent 7b9fa5586becc9dbc27040e9a77ae3909936c6a8 (draft PR #122). Phase K vault work is excluded and has its own independent branch/PR. Local revalidation: 465 native tests passed, 8 opt-in modes skipped; 397 frontend tests passed across 76 files. Clippy, Rust formatting, TypeScript, production build and whitespace checks passed. Logs: `../../audit-output/phase-j-review-*.log`.

The checkpoint remains local and dependent on #122. Its remote CI, packaging and installed hands-on validation remain paused; no merge or release is requested. Historical integrated-workspace notes below retain their original context.

# Phase J phase 3 — local production integration, release acceptance pending

## First increment: workspace admission bridge

The prototype now has a read-only admission seam beside the actual workspace project service. It reads saved workspace roots from the native settings connection, reads the workspace-owned project and local per-asset root bindings in one project read transaction, and invokes the existing validated render-snapshot path. Callers cannot supply an alternate root list or portable-document access grants.

Its receipt binds workspace/project identity, exact revision, full stored document hash, full local binding hash and saved roots. It rechecks roots before returning. A final `phase_j_workspace_current` check repeats authoritative admission and compares the receipt, rejecting stale same-revision edits as well as ordinary revision changes. Existing native source fingerprint checks are retained. Multi-root projects keep a per-asset root map; no flattening into one caller-provided folder occurs.

This code is **test-only and not connected to IPC, editor controls or production rendering**. It does not hold source handles between admission and handoff; the decoder's source pins and final receipt check still need to be integrated. Two separate database connections cannot provide atomic cross-database authorization for the entire future job. The receipt is a revalidation seam, not a claim that this integration is finished.

Regressions cover wrong workspace, stale revision, same-revision document edits, revoked workspace roots, multiple source roots, removed roots, changed source bytes and preservation of unsupported future-schema documents. All project/settings databases and media fixtures are isolated.

## Workspace-to-preview bridge

`PhaseJPreviewSources` now keeps the authoritative receipt and snapshot opaque. The decoder can obtain its per-asset roots only after the entire selected manifest project matches the admitted workspace document. Both photo/video decoding and independent/linked PCM decoding use this same map. No renderer-provided roots or bindings are accepted.

`workspace_av_next` admits before decoding, retains the existing original-source pins through the returned RGB/PCM payload, and repeats authoritative admission after composition. `workspace_av_current` repeats the workspace check at consumption. The legacy `av_current` entry point rejects workspace-bound results, preventing an accidental delivery path that omits authorization. Admission and composition failures complete the worker and release its slot.

Two real-engine regressions cover a video root plus a second photo/WAV root, live source locks, final access revocation, revocation during composition, document mismatch and worker release. This joins the existing gated session adapter to workspace authorization; it does not migrate that adapter's disposable storage into production or activate IPC/editor controls.

## Local production implementation — 0.56.88

The production path is separate from the disposable prototype/session tables above. `workspace_transitions.rs` owns native IPC and an append-only proposal store beside workspace projects. A proposal contains version 1 timing decisions, exact base revision and full stored JSON hash. Its full base document is deduplicated into an immutable snapshot table in the same transaction as the revision and head. Timeline history can expire without making a proposal unrecoverable. Conflicts, unknown schema objects, unknown documents and future versions fail without replacement. No portable document supplies roots or bindings.

Canvas contains one lazily opened **Crossfade proposal** disclosure. `TransitionSession` reuses the project's serialized save/history/close guard. Source timeline edits make proposals stale; applying them to the new timeline requires an explicit rebase. Invalid frame drafts remain editable and block rendering. Portable JSON includes the complete v3 timeline plus versioned crossfade decisions. Opening it creates a new workspace project and proposal atomically. Historical recovery can restore matching edges or recover the complete original composition as a new project. Source-timeline copies are labeled separately.

The preview and export commands use the existing native video semaphore. Large source hashes run after the job ID is returned and check cancellation while reading. No second player or export owner is introduced. The proposal preview's timeline uses native frame positions, overlap rows and frame-step seeking; source mutation controls are disabled during that view. Returning to the source timeline cancels the native preview and releases source ownership.

Production rendering normalizes each clip with the existing source trim/VFR policy, encodes its nonoverlap body once and retains lossless RGB handles for adjacent overlaps. Each boundary reads two handles, blends them, applies global titles and encodes the result once. It never repeatedly re-encodes an accumulated timeline. Source pins, temporary piece pins, byte/allocation/disk budgets, cancellation and cleanup checks stay active.

RGB weights use `(frame + 0.5) / overlap_frames`. Stereo float PCM is normalized to 48 kHz, mixed with complementary `(sample + 0.5) / overlap_samples` weights, and clipped once after the complete mix. Added tracks use integer sample trims/delays and sample-centered fades. Title intervals are quantized to project frames and exclude their end frame. Dips precede global titles. Final H.264/AAC output and scaled previews are compared against reference decoded pixels/samples in synthetic real-engine regressions; this is not a claim of listening or real-footage acceptance.

Native render bindings capture authoritative settings/project database paths, workspace roots, canonical per-asset roots, project revision/hash, proposal revision/hash and complete local bindings hash. They are rechecked before preview delivery and before export publication. Windows media handles prohibit writers/deletes while rendering and remain owned through delivery/publication. Root junction retargets are rejected even when settings JSON and media bytes are identical. Files remain referenced from registered workspace roots; originals are never rewritten, and final output still uses staged no-overwrite publication.

## Activation and acceptance

Default release builds expose proposal saving/recovery but refuse crossfade playback/export. Windows debug builds and explicit `--features phase-j-preview` acceptance builds enable the render commands. `workspace_transition_capabilities` drives the UI. There is no environment-variable activation path. The existing hard-cut/dip editor remains available.

For an isolated Windows acceptance build, use the existing dependency preparation/build procedure and pass the Cargo feature through Tauri, for example `npm run tauri -- build --features phase-j-preview`. Do not install into a personal profile. Use disposable Windows runners and preserve test evidence and exact commit identity.

Remaining gates before release activation:

1. Commit the reviewed implementation and obtain exact-head frontend/native CI and full Windows packaging results. Earlier PR #122 / CI #1437 cover 0.56.87, not these local changes.
2. Run disposable installed application checks for proposal persistence/recovery, old/new project interchange, cancellation/closing, workspace revocation, preview/export parity, update/default-uninstall retention and explicit cleanup.
3. Complete installed keyboard/screen-reader, real-footage/listening, unavailable/reconnected/physical-drive and signed upgrade acceptance. Short synthetic tests do not establish those results.
4. Review evidence, then explicitly enable release rendering. Do not remove the gate merely because local tests pass.

The local implementation does not imply completion of these release gates. This increment has not been pushed, packaged or installed. Local check evidence is recorded below when complete.

## Local verification — 2026-11-07

| Check | Result | Local evidence |
| --- | --- | --- |
| Full native suite | 465 passed; 8 opt-in modes ignored | `../../audit-output/phase3-local-native.log` |
| Clippy, all targets, warnings denied | Passed | `../../audit-output/phase3-local-clippy.log` |
| Rust formatting, including included modules | Passed | `../../audit-output/phase3-local-format.log` |
| Full frontend suite | 397 passed across 76 files | `../../audit-output/phase3-local-frontend-final.log` |
| Latest editor/session/service regressions | 35 passed, including StrictMode | `../../audit-output/phase3-editor-services-final.log` |
| TypeScript and production Vite build | Passed; existing large-chunk advisory remains | `../../audit-output/phase3-local-types.log`, `phase3-local-build-verified.log` |
| Browser layout and interaction fixtures | Light/dark at 1220×780 and 800×580; no page errors or horizontal overflow; invalid drafts retained; shared read-only overlap timeline and return-to-source verified | `../../audit-output/phase3-editor-browser.json` and `phase3-editor-*.png` |

Browser checks use isolated mocked IPC/media fixtures and verify UI behavior/layout only. The preview placeholder intentionally cannot play its mocked URL; actual media rendering/parity is covered by native real-engine synthetic tests. No installed playback, listening, screen-reader, physical-drive or signed-upgrade acceptance is claimed. Earlier remote CI remains at 0.56.87 and does not validate this uncommitted 0.56.88 work.

| Before | After | Why |
| --- | --- | --- |
| Crossfade groundwork lived in disposable test adapters | Separate workspace-owned native store and gated existing-editor controls | Preserve authoritative ownership and durable recovery |
| A single video with added audio could take the source-only fast path | Layered projects use the composed renderer and editable project frame rate | Added tracks and titles must survive preview/export |
| Source timeline timing also described proposal playback | Native shortened overlap positions and one shared player | Match what is previewed/exported without duplicating project state |
| Only a project hash could survive beyond rolling history | Immutable full base snapshots accompany proposal revisions | Recover the complete composition after older main saves expire |
