# Experimental crossfade source fingerprint gate

The new test-only `crossfade_integrity::verify_source_fingerprints` verifies original source files against **previously stored** (not freshly trusted) byte lengths and SHA-256 digests via the existing cancellable `video_project::fingerprint` routine. It rejects missing source files, malformed identity records, duplicate resolved paths, cancellation, byte-size changes and same-size content mutations. Synthetic tests verify source modification and deletion.

**Not yet production-wired:** This helper must receive expectations from a validated saved sequence snapshot, and must be called **before** normalizing source media and **after** encoding and full output validation, prior to safe publication. It is not safe to generate a new expected hash after beginning an export, because that would bless already-changed media. The current test-only executor still uses normalized scratch MKVs and does not accept original source expectations. Filesystem races, secure handles and Windows reparse points require further acceptance.

The user-facing hard-cut editor and saved project schema remain unchanged.
