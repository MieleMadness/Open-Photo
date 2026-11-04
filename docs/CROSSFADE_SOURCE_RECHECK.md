# Native crossfade before/after source verification

A test-only boundary wraps the validated scratch executor with an externally supplied source verifier: it runs **before** normalized-MKV validation and FFmpeg encoding, and **after** successful full-decode output verification. If the original-source verifier reports a change or the job is cancelled after encoding, the completed scratch MP4 is deleted and the caller receives an error. The Windows fixture tests both pre-execution rejection and a post-render source change; neither can leave an output eligible for publication.

The verifier must check **stored** SHA-256/byte lengths from a validated saved project (the separate integrity gate in PR #40). A successful check does not freeze original files against filesystem replacement races. Before any real export, revalidate source handles and integrate the staged no-overwrite publisher (#41). This experimental adapter is **not** in production export or editor UI.
