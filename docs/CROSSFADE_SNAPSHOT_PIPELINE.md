# Test-only crossfade pipeline saved snapshot bridge

The experimental `render_verify_publish_snapshot` entrypoint now takes an existing validated sequence Snapshot and trusted relink roots, derives original media identities using the previously merged `SourceManifest`, and runs the integrated source-verified rendering and safe publication pipeline. It rejects normalized input counts that differ from the saved timeline clip count before any encoder invocation.

This change intentionally **does not** treat caller-provided normalized scratch MKVs as provably derived from the saved timeline. Before production export, establish authenticated part provenance, source-to-part ordering, normalized frame allocation and stable Windows handles. The native executor's input stream and frame validation remains mandatory but is not a cryptographic provenance proof.

A Windows sequence fixture tests the saved-document entrypoint rejects missing/mismatched normalized parts without creating an export. This code is test-only; no schema migration, transition UI or production export route has changed.
