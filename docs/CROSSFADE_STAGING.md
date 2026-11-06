# Experimental cancellable staged-copy primitive

A test-only byte-transfer helper copies a previously verified scratch MP4 into an already-owned staged output in bounded 64 KiB chunks. It polls cancellation before each chunk and after EOF, rejects short/long byte counts, propagates reader and writer failures, and enforces the existing maximum export byte budget. It never writes directly to the user's chosen destination.

Unit tests cover exact copies, truncated/growing sources, mid-copy cancellation, invalid byte budgets and injected read failures.

**Not yet production wired:** Future integration must replace the publisher's uninterrupted `io::copy`, preserve `safe_output::StagedOutput` RAII cleanup, and perform the existing staged SHA-256, original source fingerprint and no-overwrite checks. Cancellation is cooperative, so a blocked filesystem read/write cannot be forcibly interrupted. Windows file-handle ownership and large-project acceptance remain required.
