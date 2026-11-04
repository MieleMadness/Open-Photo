# Phase I consolidated experimental safety gate — November 4, 2026

All crossfade code remains behind `#[cfg(test)]`; no production editor transition routing or saved-schema changes.

## This branch
- Windows `PinnedMedia`: reject reparse-point files and hard links, verify file identity with the Win32 `GetFileInformationByHandle` volume/file-index fields rather than relying on unstable Rust metadata methods. Compare a separately reopened pathname to the pinned handle both before and after streaming SHA-256. Reject duplicate file identities. Keep `FILE_SHARE_READ`-only ownership throughout a render.
- Native bundled-FFmpeg normalization fixture: pin the saved original **during** the normalization encode, not merely after the normalized output was generated. Preserve the existing post-normalization receipt and crossfade output checks.
- Final MP4 staging: require enough measurable free space at the destination for the entire verified scratch-copy plus 32 MiB reserve. Refuse publication if the filesystem cannot report free space.
- Add a PR label `full-windows-ci` to request the installer and bundled-video checks in addition to ordinary native and frontend CI.

## Explicit Phase I release gates that CI cannot automatically close
1. Install the built Windows package on a real reference PC, without an external FFmpeg; open a saved two-video project, normalize, preview, render and compare actual decoded transition frames and audio samples.
2. Exercise real phone SDR and VFR clips, rotated footage, source relinking, repeated clips, Unicode and long paths, cancellation, missing media, destination collisions, and full cleanup with unchanged source hashes.
3. Test Windows local NTFS/ReFS behavior and unsupported network/share configurations; test privileged writers, directory junctions/reparse races, and permission errors. The handle checks fail closed for unsupported identities but do not protect against privileged/kernel writes.
4. Measure normalizer/export disk, peak memory and duration on the reference PC; run short fixtures now and defer the specifically requested 10,000-item library stress test to pre-1.0.
5. Treat `#[cfg(test)]` as a security boundary: production Phase J transition UI/schema/export must not be enabled until the normalizer itself mints trusted receipts and installed-app behavior passes acceptance.

## Acceptance policy
Merge only a successful exact-head workflow; if native lint, security or installer checks fail, fix on this branch and rerun. CI passing is not equivalent to installed-app signoff.
