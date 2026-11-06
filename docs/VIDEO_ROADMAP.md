## November 6, 2026 — Phase J resumed-session frame/sample seeking

Local source **0.56.83** adds gated native seek descriptions over freshly verified
selected manifests. They map video/photo parts, global title IDs and exact 48 kHz
audio slices, preserving subframe added-track edges and independent envelopes.
Frame/sample endpoints are exclusive; results bind session/project identity.
See [session seek mapping](PHASE_J_SESSION_SEEK.md).

Next: bounded preview requests, cancellation and delivery-time stale-result
rejection, then gated decode/composition integration. This mapper performs no
playback or mixing. Native linear versus older experimental equal-power audio,
compositor policy, physical source verification, migration/import and real-footage/
installed-Windows parity remain gates. No active production preview/export,
install, push, upload or remote CI is implied. XMP's 0.56.76 handoff is preserved.

---

## November 6, 2026 — Phase J saved session checkpoints and recovery

Local source **0.56.82** adds gated append-only checkpoints, explicit latest
resume and historical recovery into a separate session. Selected and inactive
history manifests are revalidated; expected latest receipts reject conflicting
saves, and stable recovery selections avoid duplicate copies after lost replies.
Three process-exit trials distinguish committed checkpoints from retained but
uncheckpointed edits. See [session checkpoints](PHASE_J_SESSION_CHECKPOINTS.md).

Next: shared preview/seek frame and sample mapping for resumed mixed-layer plans.
Edit and checkpoint remain separate test-only operations; production scheduling,
retention, migration/import, authorization/source verification and real-footage/
installed-Windows parity remain gates. No active production editor/render route,
install, push, upload or remote CI is implied. The XMP 0.56.76 handoff is preserved.

---

## November 6, 2026 — Phase J gated editor sessions and undo/redo

Local source **0.56.81** adds test-only session selection, revision-checked
crossfade edits and undo/redo over immutable mixed-layer manifests. Current and
target snapshots are revalidated, no-ops retain history, new branches clear redo,
and failures preserve session state until commit. Undo retains 20 changes without
deleting snapshots. See the [editor-session contract](PHASE_J_EDITOR_SESSION.md).

Next: durable session checkpoints and explicit resume/recovery, then shared
preview/seek mapping for selected mixed-layer plans. Current session history is
in memory, and no production editor/command/rendering route is enabled.
Storage/import/authorization, source verification, preview/export parity and
real-footage/installed-Windows acceptance remain gates. No install, push, upload
or remote CI is implied; the XMP 0.56.76 handoff is preserved.

---

## November 6, 2026 — Phase J gated immutable mixed-layer manifests

Local source **0.56.80** saves versioned mixed-layer manifests and revalidates
them against the actual native project plus a recomputed frame/sample plan.
Immutable content IDs preserve different proposals and make unchanged retries
duplicate-free. Same-revision metadata edits, corrupt/future records and unknown
schemas fail while stored records remain intact. See the [manifest contract](PHASE_J_LAYER_MANIFEST.md).

Next: gated editor-session selection, revision-checked edits and undo/redo using
these manifests. There is no active editor head or mixed-layer recovery UI yet.
Production storage/import/authorization, source verification, preview/export
parity and real-footage/installed-Windows gates remain pending. No install,
upload, push or remote CI is implied; the XMP 0.56.76 handoff is preserved.

---

## November 6, 2026 — Phase J gated mixed-media/layer clock

Local source **0.56.79** implements proposed photo/title/audio/dip timing in a
test-only native planner, with a read-only saved-project binding. Photos use the
same cumulative frame allocation; titles and added audio keep absolute times;
dips and overlaps share a two-clear-frame budget. Layer overruns and conflicting
transitions fail explicitly. See [layer timing rules](PHASE_J_LAYER_TIMING.md).

Next: define a versioned mixed-layer manifest and stale-plan validation before
editor/undo integration. The existing v1 snapshot storage still refuses mixed
layers. Compositor order, source verification, preview/export parity, listening
and real-footage/installed-Windows acceptance remain production gates. No active
crossfades, installation, upload or remote CI is implied.

---

## November 6, 2026 — Phase J gated storage/recovery contract

Local source **0.56.78** adds component-versioned disposable draft/history/copy
storage, payload checksums, bounded history reads and explicit recovery into a
separate copy. Unknown/unversioned/partial schemas are retained and refused;
selection changes invalidate recovery, and lost-reply retries return one copy.
Seven abrupt child-process exits exercise draft/copy transaction boundaries.
See the [storage/recovery contract](PHASE_J_STORAGE_RECOVERY_CONTRACT.md).

This remains `cfg(test)` groundwork with no production migration or activation.
Next: define the overlap clock for photos, titles, audio and existing dips, then
extend native binding tests. Authorized production storage/import, editor/undo,
preview/export parity and real-footage/installed-Windows gates remain pending.
The XMP 0.56.76 handoff is preserved; no install, upload or remote CI is implied.

---

## November 6, 2026 — Phase J native saved-project binding groundwork

Local source **0.56.77** adds a gated adapter backed by the actual sequence reader,
storage schema and frame allocation, plus disposable file-backed draft/history
tests. It binds drafts to saved root/project, complete stored JSON hash, native
source descriptors and project/draft revisions. See [Phase J saved-project binding](PHASE_J_SAVED_PROJECT_BINDING.md).
This is `cfg(test)` groundwork: no production migration, active transition UI,
preview/export routing, installation, upload or remote CI is implied.

Next: review production storage/old-client/recovery policy and the overlapping
clock's treatment of photos, titles, audio and existing dips. Those cases are
currently rejected by the experimental adapter rather than partially copied.
The real-footage and installed-Windows production-enablement gates remain in force.

---

## November 6, 2026 — local XMP closeout and next coding track

Local candidate **0.56.76** implements reviewed no-overwrite Apply, persisted
explicit recovery, full-source bindings, complete intent rechecks, background
workers, paged/offline review and staging observations. See [XMP closeout](XMP_CLOSEOUT.md)
for current evidence and outstanding installed/durability gates. No remote CI,
merge, installation or publication is implied by local verification. The dated
PR states and old ignored race gates below are historical, not current status.

The next coding track is **Phase J production crossfade groundwork**: review
production schema/recovery and source binding, then integrate editor/undo,
preview-clock and native-export parity. Existing hard-cut/dip behavior remains
the supported fallback. Do not enable production crossfades or release the XMP
candidate without their respective acceptance gates. The owner-deferred 10,000+
media stress test remains a pre-1.0 requirement, not an XMP feature to add now.

---

## November 5, 2026 — Windows XMP sharing experiment (PR #115 pending)

[PR #113](https://github.com/MieleMadness/Open-Photo/pull/113) merged after passing native/frontend CI, adding link-alias recovery tests. Follow-up [PR #115](https://github.com/MieleMadness/Open-Photo/pull/115) characterizes Windows share modes and ReplaceFileW with synthetic files; it is **not** a production external-editor race fix. [Issue #90](https://github.com/MieleMadness/Open-Photo/issues/90) remains open. See [sharing experiment notes](XMP_WINDOWS_SHARING_EXPERIMENT.md). No Windows installer requested.

---

## November 5, 2026 — Recovery status foundation (PR #111 pending)

**Merged:** [#110](https://github.com/MieleMadness/Open-Photo/pull/110) passed exact-head frontend/Rust CI and extracted symlink-aware read-only XMP recovery inspection. **Pending:** #111 adds internal conservative recovery-entry classification (clear, temp, backup, both, unexpected) and tests; the production writer keeps blocking any non-clear entry and does not auto-select versions. [Issue #90](https://github.com/MieleMadness/Open-Photo/issues/90) remains open for Windows-safe successful-publication conflict prevention, crash/power-loss durability, and installed manual acceptance. Routine checks run without an installer.

---

## November 5, 2026 — XMP recovery-readiness follow-up (PR #110 pending)

**Verified and merged:** [#108](https://github.com/MieleMadness/Open-Photo/pull/108) (publish succeeded, acknowledgement failed) and [#109](https://github.com/MieleMadness/Open-Photo/pull/109) (ambiguous temp/backup artifacts with later external edits) passed exact-head frontend and Rust checks; neither adds automatic recovery or closes [Issue #90](https://github.com/MieleMadness/Open-Photo/issues/90). **Pending:** [#110](https://github.com/MieleMadness/Open-Photo/pull/110) extracts a read-only recovery-directory-entry inspection helper into production without changing its blocking policy; CI and review required before merge. No Windows installer requested. The compare-to-publish race and interrupted-write durability remain open.

---

## November 5, 2026 — Consolidated 0.56.69 development and XMP integrity gate

**Current merged application baseline:** `0.56.69` on `feature/desktop-v0.1.0`. Production recovery-directory-entry checking in [#98](https://github.com/MieleMadness/Open-Photo/pull/98) passed [full Windows CI #1386](https://github.com/MieleMadness/Open-Photo/actions/runs/42093820209), including installer/export/AI/uninstall automation. Later [#101](https://github.com/MieleMadness/Open-Photo/pull/101) added publish-error regressions and parallel-test fixture isolation; [#104](https://github.com/MieleMadness/Open-Photo/pull/104) added the [manual Windows XMP safety runbook](XMP_WINDOWS_INSTALLED_ACCEPTANCE.md). These are merged, not public releases. [#106](https://github.com/MieleMadness/Open-Photo/pull/106) passed [CI #1400](https://github.com/MieleMadness/Open-Photo/actions/runs/42109846048) and merged an **ignored, intentionally failing** demonstration of the successful-rename external-editor race; **not** a fix or a passing release gate.

| Next priority | Current state | Required next action |
| --- | --- | --- |
| Issue #90 compare-to-publish integrity | Still unsafe in a deterministic external-edit interleaving; ignored red gate PR #106 pending | Select and verify a Windows-safe publication/identity protocol, then turn regression into passing CI |
| Persisted interrupted-write recovery | Recovery artifacts exist; no persisted crash-state machine or automatic ambiguity resolution | Prove stage/flush/publish/restart transitions with reversible, user-reviewable states |
| Installed Windows XMP conflict/alias tests | Runbook merged; tests **not performed** | Exercise controlled external-editor concurrency, junction/reparse/root-revocation, verify original-media hashes |
| Phase J production crossfades | Experimental/test-only; no persisted production renderer | Gate active UI/schema/preview/export on native parity and installed acceptance |
| 10,000+ asset stress test | Owner deferred | Run before version 1.0 |

**CI policy:** [#102](https://github.com/MieleMadness/Open-Photo/pull/102) requires opt-in `full-windows-ci` PR label or `workflow_dispatch full_windows=true` for installers; default PR and push verification does not produce an installer. This avoids unnecessary packaging but is not a substitute for installed Windows safety trials. Earlier dated sections below are historical checkpoints, **not the current PR or release status**.

---

## November 5, 2026 — CI cost and opt-in installer policy

**Merged development baseline:** 0.56.69, verified via full Windows [CI #1386](https://github.com/MieleMadness/Open-Photo/actions/runs/42093820209) and merged [PR #98](https://github.com/MieleMadness/Open-Photo/pull/98). Full Windows validation remains available, but the workflow now requires an explicit `full-windows-ci` label or manual `full_windows=true` input. Routine PRs and pushes no longer build installers. This reduces unnecessary CI cost without treating a checks-only green run as installed-app validation.

**Integration policy:** Require exact-head green frontend and native Rust checks for normal merges. Run the full installer/export/installed-AI/uninstall gate when explicitly requested, for release candidates, packaging/toolchain changes, or relevant installed-app safety investigations. Installed real-media/manual XMP conflict and Phase J acceptance remain distinct gates, not implied by automated CI. The 10,000+ library stress test remains pre-1.0.

---

## November 5, 2026 — 0.56.68 XMP safety checkpoint and Phase J boundary

**Current merged development baseline:** **0.56.68** on `feature/desktop-v0.1.0`. [PR #93](https://github.com/MieleMadness/Open-Photo/pull/93) passed exact-head [full Windows CI #1375](https://github.com/MieleMadness/Open-Photo/actions/runs/42081606891) and merged the XMP batch entry/path and per-item error hardening. PRs [#91](https://github.com/MieleMadness/Open-Photo/pull/91) and [#92](https://github.com/MieleMadness/Open-Photo/pull/92) previously merged documentation and foreign-metadata preservation tests. [PR #89](https://github.com/MieleMadness/Open-Photo/pull/89) was closed unmerged after being superseded by #93. [PR #95](https://github.com/MieleMadness/Open-Photo/pull/95) carries additional stale-snapshot/recovery-artifact tests and requires its own exact-head CI; no public release publication or interactive installed-Windows acceptance is claimed.

**Unresolved priority F07/R01:** [Issue #90](https://github.com/MieleMadness/Open-Photo/issues/90) tracks the external-editor **compare-to-publish race**, persisted interrupted-write recovery, Windows reparse/junction/root authorization, and no-silent-overwrite guarantees. The [XMP fault matrix](AUDIT_XMP_RECOVERY_GATE.md) documents the required deterministic tests and installed Windows trials. Existing test fixtures establish only pre-publish conflict detection and recovery-file refusal, not complete transactional safety. Do not close F07/R01 or report a production metadata conflict UI until these cases pass.

| Immediate priority | Current verified boundary | Next work |
| --- | --- | --- |
| 0.56.68 XMP batch safety (#93) | Merged after passing full Windows CI #1375 | Retain the guarded batch implementation; complete installed real-media/junction acceptance |
| External-editor and crash recovery (#90) | Native regression foundation from #88 | Deterministic compare-to-publish race, backup/flush/rename failure injection and persisted, user-reviewable recovery |
| Installed Windows acceptance | Automated checks do not exercise GUI editor flows | Real-media sidecar conflict, root/junction safety, settings/RAW and update lifecycle trials |
| Phase J production crossfades | Experimental/test-only snapshot/clock/render infrastructure | Remains gated on production schema/recovery, real-media playback/export parity and installed acceptance |
| 10,000+ media scale test | Deferred by owner | Complete before 1.0, not a prerequisite for narrow 0.56.x safety changes |

**Non-regression rule:** Original source media stays read-only, managed/foreign metadata is never silently discarded, and existing hard-cut/dip video behavior remains the production fallback.

---

## November 5, 2026 — Phase J persisted-draft safety checkpoint (experimental)

**Verified and merged:** PRs [#66](https://github.com/MieleMadness/Open-Photo/pull/66), [#68](https://github.com/MieleMadness/Open-Photo/pull/68), [#69](https://github.com/MieleMadness/Open-Photo/pull/69), [#70](https://github.com/MieleMadness/Open-Photo/pull/70), [#71](https://github.com/MieleMadness/Open-Photo/pull/71), [#72](https://github.com/MieleMadness/Open-Photo/pull/72), [#73](https://github.com/MieleMadness/Open-Photo/pull/73), [#74](https://github.com/MieleMadness/Open-Photo/pull/74), [#75](https://github.com/MieleMadness/Open-Photo/pull/75), [#76](https://github.com/MieleMadness/Open-Photo/pull/76), [#77](https://github.com/MieleMadness/Open-Photo/pull/77) and [#78](https://github.com/MieleMadness/Open-Photo/pull/78) added *isolated* command/history, frame-preview seek, source/revision-bound draft snapshots, Rust wire validators, and in-memory SQLite atomic/CAS/recovery test fixtures. Combined CI verification is still required after the latest base merges.

**Current validation:** [PR #79](https://github.com/MieleMadness/Open-Photo/pull/79) tightens native rational source-range checks for zero-length/reversed/malformed trims; exact-head CI is required before integration.

| Phase J workstream | State | Next release gate |
| --- | --- | --- |
| Crossfade editing primitives | Pure/test-only merged | Integrate in active timeline with conflict messaging and feature-gated undo/autosave |
| Preview seek clock | Pure/test-only merged | Real interactive preview parity with native exported frames/audio |
| Versioned draft and source binding | Pure/test-only merged | Review schema migration, native source verification and crash recovery |
| Atomic save/CAS | In-memory SQLite tests only | Real production DB migration, authoritative revision checks, rollback/recovery and old-client compatibility |
| Native snapshot parsing | Rust test-only merged; rational trim hardening in PR #79 | Verify exact native source fingerprints and source-mark invariants |
| Native crossfade render/export | Experimental/test-only | Real installed Windows SDR/VFR playback and export acceptance, safe fallback, no-overwrite checks |

**Do not report production crossfades as enabled.** No user-facing transition controls, persistent schema migration, live preview or native export routing has shipped via these PRs. Existing hard-cut and dip-to-black behavior remains the supported path. Exact-head CI is an engineering gate, not a substitute for the real installed Windows acceptance report.

---

## November 4, 2026 — Phase J preview-seek frame mapping (experimental)

The `crossfadePreviewSeek` adapter maps a project playhead onto the discrete 30/60 FPS crossfade frame clock, returning source-relative trimmed frame offsets and the existing sampling weights. The project end is exclusive; no duplicate final frame is generated. Focused tests cover overlaps, hard-cut boundaries, fractional seeks and invalid positions.

This is a **pure, test-only adapter**. It does not enable production UI, preview, persistence, native rendering or export. Installed Windows real-footage acceptance and preview/export parity remain required.

---

## November 4, 2026 — Phase J isolated transition-edit command groundwork (draft)

The `crossfadeCutCommands` module adds immutable add/replace/remove operations for an **experimental** adjacent-cut crossfade definition. It validates saved clip identities, 30/60 fps whole-frame bounds, two-frame reserve on both sides, and any neighboring overlaps *before* changing project state. No-op edits signal `changed: false` so a later UI can avoid unnecessary undo/autosave history, and changed boundaries are serialized in clip order. Focused unit tests cover frame budgets, malformed durations, removal and stale cuts.

**Boundary:** This patch does not persist crossfade transitions, add controls, change schema v3, or route production preview/export. It is not user-visible. The accepted hard-cut/dip editor remains unchanged. Production Phase J is blocked on a real-media Windows acceptance report, production schema/recovery design, editor parity and verified export routing.

**CI note:** Validate this PR's exact head before merging; an independently green branch is not sufficient to enable production crossfade export.

---

## November 4, 2026 — Phase I native engineering checkpoint (updated)

**Automated engineering baseline:** The experimental crossfade native pipeline was integrated through [PR #61](https://github.com/MieleMadness/Open-Photo/pull/61), which passed exact-head Windows Rust, frontend and full installer CI. [PR #63](https://github.com/MieleMadness/Open-Photo/pull/63) adds verified Windows file identity, hard-link rejection, pinning of saved originals during normalization and a final destination-space recheck; it requires its own full exact-head Windows success before merge.

**Phase I distinction:** The *experimental, test-only crossfade infrastructure* may clear its automated development gate without crossfades being shipped. There is no active production transition schema, timeline control, real-time preview, or export route. The existing hard-cut/dip editor remains the production fallback.

**Open verification:** Installed Windows interactive editing with real phone SDR/VFR media, external-codec independence, source relink/cancellation, filesystem reparse/privileged-race behavior, user-visible export and performance measurements are still unverified. See [Phase I exit gate](PHASE_I_EXIT_GATE.md).

**Phase J entry criteria:** Only begin production schema/UI/preview/export routing after #63's exact-head green checks and a reviewed real-footage acceptance report. Implement schema migration, UI transition creation/undo, synchronized preview/export frame clock, and active export with safe fallback under separate gated PRs. Do not imply that full Windows CI's installer checks exercised interactive crossfades—they remain test-only.

---

## November 4, 2026 — Phase I crossfade development checkpoint

**Development baseline:** `feature/desktop-v0.1.0`; the documented application version remains **0.56.63**. This section tracks engineering and test readiness, **not shipped crossfade support**.

| Workstream | Verified development state | Next requirement |
| --- | --- | --- |
| Existing video editor | Hard-cut/dip rendering, waveform and timeline navigation foundations integrated; do not regress preview/export | Installed Windows editing acceptance and source preservation |
| Crossfade project clock | TS/Rust frame-clock models, trimmed source adaptation, native preflight and FFmpeg graph planning merged through PRs #21–#29 | Audio sample parity and true preview/export clock consistency |
| Native normalized inputs | [PR #30](https://github.com/MieleMadness/Open-Photo/pull/30) merged after exact-head Windows CI #1178; test exercises existing part normalization with a source lacking audio | Validate output exact media frame/sample counts, source fingerprints, cancellation and cleanup |
| Mixed transition graph | [PR #31](https://github.com/MieleMadness/Open-Photo/pull/31) merged after exact-head Windows CI #1181; verifies overlap followed by hard-cut graph ordering | Add real bundled-engine mixed-order and audio acceptance |
| Crossfade resource guards | [PR #32](https://github.com/MieleMadness/Open-Photo/pull/32) merged after exact-head Windows CI #1184; two-hour and 200-clip boundary tests | Enforce these bounds in later native execution |
| Bounded FFmpeg command | [PR #33](https://github.com/MieleMadness/Open-Photo/pull/33) merged after exact-head Windows CI #1187; bounded test-only command planning with no shell or user export routing | Validate scratch ownership and trusted normalized streams |
| Windows synthetic execution | [PR #34](https://github.com/MieleMadness/Open-Photo/pull/34) merged after exact-head Windows CI #1193; bundled-engine synthetic fixture verifies frame/audio timing and byte-level no-overwrite | Verify cancellation and decoded audio samples |
| Crossfade scratch ownership | [PR #36](https://github.com/MieleMadness/Open-Photo/pull/36) merged after exact-head Windows CI #1197; tests canonical scratch part/output ownership | Link to verified normalizer, test reparse-point races and cleanup on cancel |
| Safe native execution | [PR #37](https://github.com/MieleMadness/Open-Photo/pull/37) merged after exact-head Windows CI #1207; cancellable scratch rendering, partial-file cleanup and full MP4 decode | Integrate strict input/source checks, real installed media and post-encode verification |
| Normalized stream contract | [PR #38](https://github.com/MieleMadness/Open-Photo/pull/38) merged after exact-head Windows CI #1209; per-part clock/canvas/video/audio validation | [PR #39](https://github.com/MieleMadness/Open-Photo/pull/39) merged after exact-head CI; explicitly counts decoded MKV frames before graph execution |
| Original source integrity | [PR #40](https://github.com/MieleMadness/Open-Photo/pull/40) merged after exact-head CI; verifies previously saved SHA-256 and byte length including same-size mutations | Integrate before normalizing and before publishing |
| Verified output publication | [PR #41](https://github.com/MieleMadness/Open-Photo/pull/41) merged after exact-head CI; reuses staged no-overwrite export with source verification callbacks | Enforce real snapshot verification, decode and stable file ownership |
| Decoded audio sample clock | [PR #42](https://github.com/MieleMadness/Open-Photo/pull/42) merged after exact-head Windows CI; AAC presentation sample clock accounts for priming/padding, with bundled 2-second acceptance fixture | Bounded-memory streaming audio packet inspection for long projects |
| Before/after source recheck | [PR #44](https://github.com/MieleMadness/Open-Photo/pull/44) merged after exact-head Windows CI; discards scratch MP4 when sources change before/after experimental render | Wire verified saved snapshot sources and secure handle ownership |
| Independent audio integrity | [PR #45](https://github.com/MieleMadness/Open-Photo/pull/45) merged after exact-head Windows CI; revalidates used standalone audio tracks after preview | Installed Windows editing acceptance and source preservation |
| Verified experimental pipeline | [PR #46](https://github.com/MieleMadness/Open-Photo/pull/46) merged after exact-head Windows CI (source hashes bound to no-overwrite publication); [PR #47](https://github.com/MieleMadness/Open-Photo/pull/47) merged after exact-head Windows CI; chained synthetic normalized input validation, decode, AAC clock, original verification and publication | [PR #48](https://github.com/MieleMadness/Open-Photo/pull/48) and [PR #51](https://github.com/MieleMadness/Open-Photo/pull/51) merged: saved-project source manifest and experimental bridge; still require proven input normalization provenance and installed Windows acceptance |
| Verified cancellable staging | [PR #52](https://github.com/MieleMadness/Open-Photo/pull/52) merged after exact-head Windows CI; [PR #53](https://github.com/MieleMadness/Open-Photo/pull/53) merged to restore pinned SFace model CI downloads; [PR #54](https://github.com/MieleMadness/Open-Photo/pull/54) merged after exact-head Windows CI; cancellable 64 KiB staging, original SHA-256 checks, scratch/stage fingerprints and atomic no-overwrite commit | Confirm verified native pipeline, Windows source/part pinning and installed-media acceptance |
| Provenance at normalization boundary | [PR #55](https://github.com/MieleMadness/Open-Photo/pull/55) merged after exact-head Windows CI; verifies ordered saved-source and normalized-MKV fingerprints. [PR #56](https://github.com/MieleMadness/Open-Photo/pull/56) merged; real FFmpeg normalization fixture mints ordered receipts. [PR #57](https://github.com/MieleMadness/Open-Photo/pull/57) merged; verified pipeline rechecks receipt hashes before/after rendering | [PR #58](https://github.com/MieleMadness/Open-Photo/pull/58) merged Windows file-handle guard; #60/#61 cover retained lifetime and remaining native hardening |
| Phase I consolidated native acceptance | [PR #60](https://github.com/MieleMadness/Open-Photo/pull/60) open: retains verified original and normalized handles through a real FFmpeg fixture and publication; [PR #61](https://github.com/MieleMadness/Open-Photo/pull/61) open: reparse-point checks, preflight disk budgets, constant-memory AAC packet inspection, and full saved-project publish regression | Exact-head Windows CI, installed Windows reference-machine tests, trusted normalizer ownership and remaining pathname/hard-link races before declaring the full Phase I acceptance gate complete |
| Real crossfade functionality | **Not enabled**; no live transition selection, project schema migration, or active preview/export integration | Strict cancellable native render path, verified frame/audio output, schema and UI with undo/reopen parity |
| Other roadmap gates | 10,000+ real-media scale test deferred to **pre-1.0**; music licensing, private vault and installer signing remain separate | Dedicated hardware, release and security acceptance |

**CI integration policy:** Merge only exact-head green PRs into the development branch. Work on unrelated feature branches while CI runs, but never interpret a previous green SHA as approval for a newer commit. Treat stacked PRs as dependent and retarget/revalidate after their parent merges. Fast PR CI does not establish full installer acceptance. Keep the existing source read-only and existing hard-cut renderer as fallback until crossfade verification is complete.

**Immediate sequence (updated November 4, 2026):** [PR #61](https://github.com/MieleMadness/Open-Photo/pull/61) merged with exact-head full Windows installer CI, bounded-memory AAC packet accounting and native end-to-end publication regression. Validate [PR #63](https://github.com/MieleMadness/Open-Photo/pull/63)'s Windows identity, normalization pinning and post-render staging-space protections with its *own* full exact-head Windows workflow. Use the [real-media acceptance record](PHASE_I_REAL_MEDIA_ACCEPTANCE.md) for installed Windows testing. The active user-facing transition schema, timeline editing, preview and export routing belong to **Phase J**, not Phase I.

---

> **0.56.21:** The shared timeline now accepts videos plus JPEG/PNG/BMP/TIFF photos and produces one continuous output, including photo-only projects. Use **Add media…**, select a photo to change its duration or shared image adjustments, and **Preview project edits** to render the saved sequence. Schema 3 stores typed assets and migrates older video documents. Text, local music and transitions remain pending; Phase F video semantic indexing is the next core search milestone. Short fixtures remain the baseline.

> **0.56.8:** One project now previews and exports its entire ordered timeline as one continuous MP4. Start in the Library with one video's Edit action or **Create video from selection…**, then **Add videos…** to the same project. Select clips to trim/adjust, split at the playhead, drag/reorder, delete with the gap closed, and undo/redo. Preview playback follows the project playhead; **Show original source** restores source playback. Canvas shape/fit are saved edits; Project / up to 1080p / up to 4K are independent export choices. Joined clips use 30/60 fps with linked original audio and silence for silent/muted intervals; one clip retains source timing. Exports create new files and never overwrite existing destinations. Missing unused bin assets do not block output. Photos will join this same editor later. See [continuous rendering](VIDEO_SEQUENCE_RENDERING.md).

> **0.56.7:** Workspace-owned persistence, cross-folder video sources, safe legacy migration, and bounded recovery are implemented. Single/multi editing now uses one controller and timeline. Continue with one compiled continuous preview/export path, then photo clips.

> **0.56.6:** Open the editor from the Library. Select several videos from one source folder to create one ordered project. Timeline clips can be selected, trimmed, split at the playhead, reordered by dragging or Move earlier/later, deleted with the gap closed, and appended again from the bin. Undo/redo and autosave apply to these commands. Empty timelines keep their assets. The player currently shows an original source; assembled playback/export and unified workspace-owned editing are next. Photos are planned as clips in this same project, not separate projects. Long-form work is deferred to 1.x.

# Video editing roadmap

Status: Phase 6 core features remain available. Version 0.56.2 passed continuous 64-minute native export, full decode, source preservation, distant frame/timestamp checks, and sampled listening checks. Phase 7 starts in 0.56.3 with multi-video project documents and a durable media bin; timeline assembly/preview and combined export follow. Short clips are the development test baseline. Further long-form testing and optimization are deferred to 1.x at the owner's request. Installed-package GUI/playback and whole-app performance acceptance remain open; physical drive/disk failures are explicitly deferred to the pre-v1.0 release checklist. HDR tone mapping and waveforms remain deferred. See [VIDEO_ACCEPTANCE.md](VIDEO_ACCEPTANCE.md).

## Phase 6 — Single-source video editing

Goal: open one video, make basic nondestructive edits, save the project, and export a new video reliably. Keep one source asset per project in this phase; the project schema should already support multiple clip instances for Phase 7.

### 6.1 — Media engine and capability discovery

- Add Rust `video_probe`, `video_jobs`, `video_project`, and `video_export` modules, with a shared media capability result consumed by the UI.
- Use a pinned FFmpeg/ffprobe distribution and document its included codecs and distribution requirements. Probe duration, streams, codec, dimensions, sample aspect ratio, rotation, frame-rate/time-base information, audio layout, and color/HDR characteristics.
- Launch processes with argument arrays, fixed executable paths, validated local assets, bounded diagnostics, and timeouts. Do not compose shell commands from filenames or permit arbitrary network input URLs.
- Begin with MP4/MOV inputs supported by the packaged decoder set and one validated MP4 export preset. Offer clear errors for unsupported streams. Identify HDR and either implement tested tone mapping or explicitly reject HDR export in the first release.

Acceptance: corrupt files, Unicode/space-containing filenames, rotated phone footage, silent clips, variable-frame-rate recordings, and unsupported codecs return accurate capability results without changing originals.

### 6.2 — Playback, seeking, and proxies

- Stream local assets using bounded range reads; restrict access to registered project assets rather than exposing arbitrary filesystem paths.
- Create cancellable proxy jobs for media WebView2 cannot play smoothly. Track source/proxy timing and orientation so edits map back to the source.
- Add play/pause, scrubber, current time, duration, in/out markers, keyboard seeking, and a filmstrip. Add a basic waveform if it fits this milestone.
- Use exact source timestamps/rational time bases internally; avoid making floating-point seconds the authoritative edit representation. Variable-frame-rate sources require explicit preview/export timing tests.

Acceptance: a clip larger than 48 MB seeks without loading the entire source into JavaScript; switching sources cancels or invalidates old work; proxy preview and source export agree on selected boundaries.

### 6.3 — Basic edits and project persistence

- Trim beginning/end; rotate in quarter turns; flip; crop with free and common aspect presets; fit/fill output canvas; mute and volume. Include simple brightness/contrast only when preview/export agreement is tested.
- Save a versioned project with asset ID, source fingerprint, relative path plus relink information, source time range, transforms, audio gain, and output settings. Never store edits only as UI state.
- Provide undo/redo, reset, autosave, recovery after restart, and explicit relink when a source moves. Keep unsaved-edit behavior clear when closing or switching media.
- Separate source metadata, editable project state, and derived caches. A changed source invalidates derived caches and requires project revalidation.

Acceptance: reopen a saved project and obtain the same edit; undo restores prior values; moving a source prompts relink; original hashes stay unchanged. One source with one kept range is enough for the first milestone. Interior cuts and repeated clip instances can follow in Phase 7.

### 6.4 — Reliable export

- Export accurate trims through decode/re-encode by default. A later stream-copy fast path must advertise keyframe limitations and only activate for eligible edits.
- Start with one tested SDR MP4 preset, original-size or 1080p options, aspect-preserving dimensions, and intentional audio mapping. Do not silently discard extra streams or HDR metadata without explaining the chosen output policy.
- Expose progress, elapsed time, cancel, and useful errors. Write to a unique temporary output; validate the result; publish to the destination with no-overwrite semantics. Preserve completed user files and clean only job-owned temporary files.
- Add one-job-at-a-time scheduling initially. Detect insufficient disk space where feasible and handle write failures without leaving a final-looking broken export.

Acceptance: exported duration and trim boundaries match the project within a documented decoded-frame tolerance; A/V synchronization passes short and long fixtures; cancellation terminates the child process; source and existing destination files remain unchanged; installer contains the required engine and works without a separately installed FFmpeg.

An automated optimized 1080p export baseline now runs in CI and produces timing/native-child memory reports. See [repeatable acceptance](VIDEO_ACCEPTANCE.md) for its scope and remaining manual gates.

### Phase 6 completion gate

Before v1.0, all current CI gates plus real packaged-Windows tests must pass. Physical drive-removal and actual disk-full acceptance are deferred to that pre-v1.0 checklist. Long-form content work resumes in 1.x; use short clips for current development. Cover silent/audio clips, portrait rotation, variable frame rates, codec fallback, long paths, source removal, cancellation, disk-full failure, reopen/relink, and output frame checks. Record startup, seek latency, export speed, and peak memory on a named reference PC. Choose numeric performance budgets after the initial baseline rather than promise unmeasured responsiveness.

Deferred: multiple source assets, transitions, titles, background music, overlays, speed ramps, keyframes, auto captions, and AI video effects.

## Phase 7 — Basic multi-video timeline

Goal: assemble several videos into one coherent output with familiar drag, split, trim, reorder, and export interactions. The first delivery needs a single sequential video track with linked source audio, not a full compositing editor.

### 7.1 — Multi-asset projects and migration

Implemented first increment (0.56.3): a separate Video projects page, schema v2, 1–50 verified source assets, stable clip references, import of v1 files into new drafts, revision-checked autosave/reopen, project copies, per-video source status and relinking, and duplicate import detection. Source files and existing v1 drafts remain unchanged. Projects currently use one selected library folder and its subfolders; sources across unrelated roots, assembled preview, and combined export are not included yet.

Next increments, in order: (1) timeline ordering/split/trim/ripple delete and undo; (2) continuous mixed-media preview; (3) combined export; (4) recovery/relink/keyboard polish and short-content release acceptance.


- Migrate Phase 6 projects into a multi-asset schema without losing edits.
- Each clip instance has an immutable ID, asset reference, source in/out, timeline placement or sequence order, transforms, and audio settings. One asset can appear repeatedly.
- Keep source time and timeline time distinct. Define one project canvas, frame rate, and audio output format. Retain source timing metadata for conversion.
- Add a media bin, multi-file import, missing-source relink, and duplicate-import detection.

Acceptance: migrated single-video projects export identically; repeated use of one asset survives save/reopen; source relink updates all relevant instances intentionally.

### 7.2 — Sequential timeline editing

- Drag clips into a sequence; reorder; trim both ends; split at playhead; delete with ripple; duplicate; zoom timeline; snap to boundaries. Preserve linked audio during edits.
- Define MVP deletion as ripple deletion so the single-track model cannot accidentally create unexplained gaps. Make edit commands transactional and undoable.
- Use a deterministic timeline reducer/command model with unit tests for split/trim boundaries, ordering, and undo/redo.

Acceptance: adjacent cuts do not introduce extra black frames or audio gaps; splitting then rejoining an untouched clip retains timing; mixed clip lengths and repeated edits save/reopen correctly.

### 7.3 — Mixed-media preview and export

- Normalize frame rates, geometry, rotation, pixel format, and audio layout for the chosen project output. Show fit/fill and background color for portrait/landscape mixing.
- Generate preview segments/proxies from the same timeline representation used by export. A chain of independent HTML video players is insufficient to guarantee continuous timeline playback.
- Use an export graph built from validated clip records. Supply silence where a clip lacks audio, reset timestamps appropriately, and verify cumulative A/V synchronization.
- Begin with hard cuts. Add a basic title card, audio fade, or one background-music track only after sequential playback/export is reliable.

Acceptance: mixed portrait/landscape, silent/audio, and variable/constant-frame-rate clips export in order with consistent canvas and duration. Test 2, 10, and 50 clips, boundary frames, long projects, cancellation, and memory. Decode and inspect output rather than only checking process exit status.

### 7.4 — Usability and release gate

- Add keyboard shortcuts, clear selected-clip state, timeline navigation, export summary, autosave/recovery, and relink across multiple assets.
- Release when real Windows users can assemble a basic video without consulting developer instructions, and project recovery/export are dependable.
- Keep transitions, arbitrary overlapping video tracks, masks, animation/keyframes, speed ramps, advanced audio mixing, and a plugin marketplace outside the initial scope.

## Future features ranked by practical value

1. Durable metadata backup/export, project recovery, and relink across photo and video workflows.
2. Complete tag editing: rename/remove with preview, undo, and preservation of unknown XMP fields.
3. Unified cancellable jobs panel for scans, AI, proxies, and exports.
4. Proxy/cache controls with disk usage and safe cleanup.
5. Cross-root duplicate review with persistent hashes and conservative identity handling.
6. Reviewed People assignments, merge/split correction, and stable identities.
7. Video poster frames, waveforms, and reusable export presets.
8. Basic titles and a single music track after Phase 7 hard cuts are stable.
9. Immich integration as a separately scoped milestone with an actual tested server version and explicit mapping review.
