# Experimental crossfade staged-content integrity

The test-only MP4 publication gate now compares SHA-256 hashes of the scratch output **before copying**, the staged output **after copying**, and the scratch output **immediately before no-overwrite commit**, not just their file sizes. This detects same-length changes to the private rendering file during the staging window, and the existing staged output guard removes incomplete temporary data on failure. Verification of original timeline media remains a separate check at both publication boundaries.

The regression modifies a scratch MP4 with a different but same-length payload during the source-verification callback, asserts publication fails, verifies that the temporary staged copy is cleaned up, then verifies the unchanged data can be published.

**Not production enabled:** This doesn't guarantee protection against adversarial file replacement between hash and commit, does not pin Windows file handles or prevent reparse-point changes, and increases I/O by reading scratch and staged bytes multiple times. Large media exports need streaming hash/copy with stable ownership and disk/cancellation budgets before enabling crossfades.
