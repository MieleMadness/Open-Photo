# Phase I — Crossfade native export exit gate (November 4, 2026)

Phase I's goal is a **test-only**, authenticated/cancellable Windows native crossfade export pipeline. It must not add user-facing transitions, modify project schema v3, or replace the existing production hard-cut/dip editor. Those tasks belong to the next phase.

## Automated exit criteria

1. FFmpeg normalized parts derive from saved project sources; mixed silent/audible audio and frame allocation are tested with the bundled engine.
2. Provenance receipts bind clip ID, saved source ID/hash, ordered normalized filename, normalized SHA-256 and allocated frames. Receipts are created in the trusted fixture after the encoder, not by arbitrary UI text.
3. Real native integration renders and verifies a scratch MP4, checks exact AAC presentation samples **as bounded-memory compact packet lines**, full decoded video, source/part fingerprints, and atomically publishes a verified copy without overwriting existing files. A second publication attempt must fail.
4. Windows pinned input handles retain read-only sharing throughout the render; ordinary writers and deletion attempts fail until the guard is released. Reparse point attributes are refused on scratch directories, input MKVs and pinned originals; leaf file pins open with `FILE_FLAG_OPEN_REPARSE_POINT`.
5. Scratch/destination disk budgets are checked before expensive rendering; cancellation and failure never publish a partial destination. Protected originals remain unchanged.
6. Exact-head Windows CI (frontend, Rust formatting, Clippy, native regression tests) must pass, with no skipped required check.

## Not established by automated code alone

- **Installed application acceptance**: the packaged Windows editor must be exercised with actual SDR phone/camera clips, portrait footage, VFR input, mixed audio and multiple transitions on a named reference PC. This includes actual playback/scrubbing, save/reopen, original preservation and one-click export behavior.
- **Untrusted caller / filesystem adversary acceptance**: a forged receipt can still be constructed by a test-only caller; the existing real normalizer fixture demonstrates creation location, not a production capability boundary. Windows directory ancestor reparse, hard-link or privileged overwrite races, network-share behavior, and destination races require further adversarial verification.
- **Production exposure**: no transition UI, persisted schema migration or active production preview/export route exists in Phase I. This is intentionally Phase J work.
- **Performance**: physical drive removal, real disk-full conditions, whole-app RAM/GPU/seek latency and the deferred 10,000-item library scale gate belong to the installed/pre-1.0 test plan.

## Manual Windows acceptance procedure

On a Windows test PC with the matching repository revision and pinned engine, run the existing short native CI/test sequence and install the packaged desktop build from a full Windows workflow. Open a copy of a project containing two or more source videos with at least one silent clip, a mixed orientation, and a VFR source. Confirm the production hard-cut/dip behavior still works, then separately inspect the experimental crossfade fixture logs for correctly decoded frame/audio counts, no-overwrite and source hashes. Exercise cancellation, missing source/relink, and output destination collisions in the installed editor's supported export path. Record tested Windows build number, GPU, drive type, files and SHA-256 results. **Do not claim user-facing crossfades are available until Phase J integrates and accepts them.**

## Current verification

This document describes implementation targets, not an unconditional sign-off. Each exact-head CI result, installed-app result, and any discovered failures must be recorded separately before changing the gate to complete.
