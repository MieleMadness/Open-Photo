# Phase J — Source-aware experimental draft identity

Crossfade clip IDs and project-frame counts alone cannot prove that the underlying footage is still the same. The pure `crossfadeSourceIdentity` module adds an ordered identity binding using clip ID, media asset ID, saved SHA-256, byte size and *exact rational source-in/source-out ticks*. This catches changes that do not alter the number of project frames.

## Behavior
- Copy and validate the ordered source binding; never mutate caller data.
- Reject missing/duplicate clip IDs, invalid source hashes, unsafe byte counts, invalid or reversed rational trims and unsupported record versions.
- Fail closed when clip order, source, content hash, byte count or exact trim ticks change.
- Allow multiple clip instances referring to the same media asset, as long as clip IDs are distinct.
- Do not claim hashing is performed by this helper: the SHA-256 value is metadata from a previously verified saved source, not a newly calculated filesystem digest.

## Production acceptance
This module **does not** read or pin original files, validate live filesystem content, migrate the current project schema, or save crossfade drafts. It is not attached to `crossfadeDraftRecord` yet. Production persistence will need versioned TS/Rust parity, revision-checked atomic writes, native revalidation of source bytes, old-draft recovery and installed Windows SDR/VFR acceptance. The current hard-cut/dip renderer remains the production path.
