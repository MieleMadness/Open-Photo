# Local coding handoff — 0.56.23

**Source push only; no release publication approval.** The user initially requested local coding, then explicitly authorized committing/pushing to GitHub and chose to check it manually. GitHub Actions monitoring, workflow dispatch, signing-key generation and installer/release publication are not part of this handoff. The last published development version was 0.56.22; manifests are prepared consistently as 0.56.23.

## Implemented scope

1. Automatic native AI indexing with durable counters, folder exclusions, paused/offline states, searchable/paged failures and retries. Manual Library Sync remains manual.
2. Wider File Details editing, taller padded previews, shared wheel zoom/pan, and simplified native-control Live/video playback.
3. Fullscreen composition with side navigation and a remembered, virtualized library filmstrip.
4. Stable/Preview channel selection, fixed signed manifest endpoints and a manual signed-Preview publication workflow. Stable's major-only policy remains intact.
6. Bounded local video-frame embeddings, resumable private staging, ranked video timestamps, Live-pair evidence and matching-time playback.

The encrypted vault (5), remaining large-library/map work and editor text/music/transitions were not implemented here.

## Architecture references

- [Automatic indexing](AUTOMATIC_AI_INDEXING.md): queue ownership, model reuse, cancellation, source safety, policy, sampling and limits.
- [Viewer](MEDIA_VIEWER.md): controls, keyboard behavior and bounded preview resolution.
- [Updater](UPDATES.md): signing/bootstrap requirements and a real two-version upgrade gate.

Primary new modules: `src-tauri/src/ai_index.rs`, `video_semantic.rs`, `update_channel.rs`, `src/useAiIndex.ts`, `AiDashboard.tsx`, `ImageViewport.tsx`, and `LibraryFilmstrip.tsx`. Existing inference/search/player/update primitives are shared; this patch does not introduce a second editor or an external AI service.

## Local checks

Final counts/results are recorded in MILESTONES.md after the last verification. Run these from the repository root with the installed Node/Rust/MSVC environment:

```text
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

The local checkout uses a shared dependency junction; this coding environment ran Vitest with `--configLoader runner --cache=false` and Vite with `--configLoader runner` to avoid writing through that junction. These are environment-specific invocation details, not product behavior.

With prepared bundled models, explicitly run this CPU acceptance (not a long recording):

```text
cargo test --manifest-path src-tauri/Cargo.toml --locked video_embeddings_resume_staging_and_publish_with_the_real_bundled_model -- --ignored --nocapture
```

Set `OPENPHOTO_AI_BUNDLE_DIRECTORY` to the prepared local model directory if needed. The test generates a 0.6-second synthetic clip and an isolated temporary database, verifies interruption/resume, vectors/publication and unchanged source bytes, then cleans its files. Other ignored long-form/installed acceptance modes stay deferred; do not run all ignored tests indiscriminately.

## Manual verification and release checklist

1. Review the pushed 0.56.23 source update against the latest GitHub branch. Any subsequent functional release needs the next patch with synchronized manifests/docs/workflow names.
2. Rerun relevant local checks after reconciliation. Preserve source safety, saved project behavior, AI opt-in and manual Sync. Verify the installed dashboard actually indexes a short real library, pauses, resumes after restart, reports failures and honors exclusions in search/People.
3. Inspect GitHub Actions for the pushed commit manually, as requested. If a job fails, read its actual failure and fix the root cause; do not claim the last 0.56.22 green run validates this new code.
4. Build/install a normal Windows package and verify both themes, minimum window/text scaling, full-width controls, photo zoom, Live/standalone playback, fullscreen neighbors and matched-time seeking. Check counters against a short mixed library and try one deliberately corrupt copy.
5. Preview updates are **code-ready, not channel-ready**. Before publication, securely configure the existing updater public/private signing secrets on a trusted machine/repository. Do not commit private keys or disable verification. A signed bootstrap installation is required for old disabled-updater builds. Only explicitly dispatch the Preview workflow after review/authorization; it tests, signs and publishes versioned assets before replacing `preview-channel/latest.json`.
6. Verify a real signed Preview-to-Preview upgrade preserving folders, projects, tags, indexes and exclusions. Test tampered signatures, offline/current checks, channel isolation/no downgrade, and active-work blocking. Local mocked updater tests do not prove signing/publication or installation works.

## Known limits / acceptance still open

- GitHub Actions for this source update is unverified here; the user will check manually. No signed installer/release was published.
- HEIC AI indexing, HDR video tone mapping and video OCR/People are unsupported. Frame sampling can miss brief objects; model/GPU quality needs real-media acceptance.
- Source counters count physical source files; a Live still/motion pair can contribute two sources. Per-frame embeddings and partial service completions do not inflate indexed-file counts.
- Preview images remain bounded; zoom is display-only and can reveal the preview's pixel limit.
- Existing JavaScript/HEIC bundle-size warnings remain. Large-library acceleration, extended map metadata/clusters, vault, text/music/transitions and long-form optimization remain separate work.
