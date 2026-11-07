# Phase J phase 3 — production integration in progress

## First increment: workspace admission bridge

The prototype now has a read-only admission seam beside the actual workspace project service. It reads saved workspace roots from the native settings connection, reads the workspace-owned project and local per-asset root bindings in one project read transaction, and invokes the existing validated render-snapshot path. Callers cannot supply an alternate root list or portable-document access grants.

Its receipt binds workspace/project identity, exact revision, full stored document hash, full local binding hash and saved roots. It rechecks roots before returning. A final `phase_j_workspace_current` check repeats authoritative admission and compares the receipt, rejecting stale same-revision edits as well as ordinary revision changes. Existing native source fingerprint checks are retained. Multi-root projects keep a per-asset root map; no flattening into one caller-provided folder occurs.

This code is **test-only and not connected to IPC, editor controls or production rendering**. It does not hold source handles between admission and handoff; the decoder's source pins and final receipt check still need to be integrated. Two separate database connections cannot provide atomic cross-database authorization for the entire future job. The receipt is a revalidation seam, not a claim that this integration is finished.

Regressions cover wrong workspace, stale revision, same-revision document edits, revoked workspace roots, multiple source roots, removed roots, changed source bytes and preservation of unsupported future-schema documents. All project/settings databases and media fixtures are isolated.

## Workspace-to-preview bridge

`PhaseJPreviewSources` now keeps the authoritative receipt and snapshot opaque. The decoder can obtain its per-asset roots only after the entire selected manifest project matches the admitted workspace document. Both photo/video decoding and independent/linked PCM decoding use this same map. No renderer-provided roots or bindings are accepted.

`workspace_av_next` admits before decoding, retains the existing original-source pins through the returned RGB/PCM payload, and repeats authoritative admission after composition. `workspace_av_current` repeats the workspace check at consumption. The legacy `av_current` entry point rejects workspace-bound results, preventing an accidental delivery path that omits authorization. Admission and composition failures complete the worker and release its slot.

Two real-engine regressions cover a video root plus a second photo/WAV root, live source locks, final access revocation, revocation during composition, document mismatch and worker release. This joins the existing gated session adapter to workspace authorization; it does not migrate that adapter's disposable storage into production or activate IPC/editor controls.

## Remaining phase 3 work

1. Replace the gated session adapter's legacy per-root project reader with a production workspace-owned session and transition store. The authorization/root-map bridge above is implemented; production editing and relinking must use it without copying projects into disposable tables.
2. Reconcile the explicit RGB/sample-centered preview policy with actual encoded export. Verify titles, PCM envelopes, timestamp/frame boundaries and clipping through end-to-end reference outputs, including real footage. The old YUV xfade route is not already equivalent.
3. Design and implement production-versioned transition storage, native ownership, import/old-client behavior and append-only recovery. Preserve existing projects and unknown/future records. Do not promote disposable test tables by renaming them.
4. Wire the shared scheduler and editor with exact frame/sample seeking, cancellation, source-change handling and accessible status/errors. Keep one state/output owner.
5. Run exact-commit CI, full Windows packaging, disposable-runner installation/upgrade/uninstall, installed keyboard/screen-reader/audio/drive acceptance and signed upgrade checks before enabling production crossfades.

Phase 3 is **not complete**. Existing production hard-cut/dip behavior remains enabled; the current installer has not been rebuilt or installed for this increment. No commit, push or publication is implied. Evidence for this increment is under `../../audit-output/phase-j-admission-*`.

First-increment verification: 441 native tests passed (eight opt-in modes skipped), including both new admission regressions; denied-warning Clippy, Rust formatting, TypeScript and diff whitespace checks passed. The initial runs exposed an identical-source fixture mistake and the distinction between structural snapshot validation and physical revalidation; both were corrected before the passing run. No installed acceptance or production activation is claimed.
