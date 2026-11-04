# Experimental crossfade saved-source manifest

A test-only `SourceManifest::from_snapshot` now converts the existing validated video sequence `Snapshot` and its relinked roots into owned original-source expectations for the native crossfade pipeline. It uses **SHA-256 hashes and byte lengths from the saved project**, never re-learns an expected hash when exporting.

- Includes distinct photo/video assets **actually referenced by clips**, only once even if a source appears repeatedly.
- Includes external music/voiceover sources **actually referenced by audio tracks**, with the same trusted audio-path checks as the existing sequence renderer.
- Rejects missing/relinked or changed sources, cancellation, duplicate identifier collisions, duplicate canonical paths and oversized manifest counts.
- Holds owned paths, saved lengths and hashes so subsequent revalidation does not depend on ephemeral borrowed project fields.

The native Windows sequence fixture now checks successful source collection and rejects a same-size media mutation. This manifest is a prerequisite for supplying real project expectations to the experimental crossfade executor; **it does not enable crossfade preview or export**. Remaining production gates include stable Windows file handles, filesystem race protection and installed-app acceptance.
