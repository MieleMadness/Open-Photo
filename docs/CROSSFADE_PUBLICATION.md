# Experimental crossfade verified publication

A test-only adapter copies a pre-validated scratch MP4 into the existing `safe_output::StagedOutput` staging mechanism and commits through the existing no-overwrite publication path. The adapter checks input file type, declared byte count, nonempty bounded output, and invokes the caller's original-source verifier **before copying and immediately before publication**. The staging output is removed on error. Destination collision is never overwritten.

The tests cover normal publication, a pre-existing destination, stale byte count, and a new destination created during re-verification. This reuses the proven publication path rather than inventing a second export rename implementation.

**Not production wired:** The caller's verifier must use the real saved project fingerprints, validate the scratch directory, fully decode and verify the MP4 and handle cancellation; staged-byte copying does not itself verify video contents or protect against all filesystem replacement races. The experimental native crossfade graph still has no editor/export route.
