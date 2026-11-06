# Phase J — Native rational source-trim validation parity

The test-only native crossfade snapshot validator now parses source-in and source-out rational marks strictly, rejects negative/malformed/leading-zero values, zero or noncanonical time bases, and trims whose end is not strictly later than their start. It compares exact integer rational values rather than rounded floating-point seconds. Any checked `u128` arithmetic overflow is rejected instead of silently wrapping.

Focused Rust regression tests include zero-length/reversed trims, mixed time bases, noncanonical decimal representations and an accepted fractional range.

This closes a source-validation gap but **does not** enable crossfade persistence or render/export. A future production implementation must confirm the source fingerprint against verified native originals, coordinate authoritative saved-project revision checks and atomic commits, handle recovery/migration, and pass installed Windows video acceptance. Extremely large mathematical rational values that overflow native checked arithmetic are intentionally rejected pending an interoperable bigint policy.
