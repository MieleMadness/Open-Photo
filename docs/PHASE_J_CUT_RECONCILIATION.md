# Phase J — Experimental crossfade transition reconciliation

This helper is preparatory engineering, not a shipped editor feature.

When clips move, split, trim, or disappear, saved cut identities may stop being adjacent or consume too many frames. `reconcileCrossfadeCuts` returns a new validated list in project order and a `removed` list that future UI should surface to the user before autosaving. It never rewrites the original clip timing, silently shrinks a fade, mutates input, or changes the existing production hard-cut/dip renderer.

## Validation

- All clip IDs and durations must form a valid baseline.
- Old transitions still joining adjacent source clips may survive.
- Any non-integer, duplicate or exhausted overlap is rejected.
- When two overlaps compete for a shortened clip, earliest cut in project order wins; the other is explicitly reported.
- The caller must present removals and put the final edit in one undo/autosave transaction. This helper does not persist transitions.

## Production gate

Schema migration, history integration, controls, live playback, native export routing, source-integrity tests, real SDR/VFR installed-Windows media acceptance and release signoff remain separate work. A green test run for this module is not production crossfade availability.
