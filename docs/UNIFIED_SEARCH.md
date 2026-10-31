# Unified discovery — Phase E

The Library has one **Search library** field. Filename, folder/path, managed XMP keywords and ratings are available without AI. With local AI on, existing OCR text and named People are included as you type. After about 400ms without typing, the complete query adds image-content matches automatically. **Search** or **Enter** runs immediately. Superseded requests are cancelled; previous completed results remain visible as updating until replacement results are ready. No search sends media or query text to a network service, modifies originals, or automatically indexes new media.

## Use

- Type words to match names, paths, tags, indexed text or named People. All unquoted words must be covered, including across different fields. Quotes group a phrase or a field value.
- `tag:family` includes that managed XMP keyword and its descendants, such as `family/holidays`.
- `person:"Alex Lee"` searches a named person already identified in the local People index.
- `path:"C:\Media\Holiday"` narrows by folder/path.
- `is:favorite`, `rating:4` and `rating:>=4` use the existing ratings; 5 stars means favorite.
- `type:photo` and `type:video` narrow media types. Filters combine with the free-text query and existing Library filters.
- Search selects **Relevance** when beginning a query. Other sort choices remain available. Clearing the field returns to normal browsing.

Tiles retain their Image / Live image / Video identity and expose match reasons on hover; list entries also show their strongest reason. File Details lists all contributing reasons. Scores describe relative relevance, not confidence percentages. Manual indexing tools are in Settings → Local content indexes; duplicate/similar review is in Folders → Library cleanup. Search tips and the old Search and library tools section were removed in 0.56.22. Empty, loading, invalid-query and partial-provider states are explicit. A provider failure does not hide valid metadata results.

## Architecture

`unifiedSearch.ts` owns parsing and deterministic ranking. Every result has a root/path media identity, media type, bounded relevance score, typed match reasons and optional matched video timestamps. Filename/path/managed-XMP/rating metadata and native evidence enter this one ranking stage. Results are intersected with the current Library snapshot, including root and modified-time checks, before other active Library filters are applied.

The strongest contribution per kind determines the main score. A bounded support bonus rewards independent evidence; repeated tags/faces/duplicate provider rows cannot inflate relevance. Exact filename and tag/person matches rank ahead of weaker semantic similarities. Semantic similarities below the initial 0.2 cutoff are excluded. This is an explainable initial weighting policy, not a calibrated probability or an AI accuracy guarantee.

`search.rs` manages one native search worker lane. Request IDs protect newer work from obsolete cancellation; pre-start cancellations are remembered in a bounded history. Queued obsolete jobs exit before inference, and running providers check cancellation between records. The current model inference may finish before cancellation takes effect. `useUnifiedSearch.ts` debounces complete queries and rejects stale responses immediately after query, workspace, source snapshot, or AI preference changes. Current completed matches remain visible while the next query runs, within the same workspace/library/AI context.

`search_providers.rs` reads the existing OCR, named-People and image-embedding stores. It obtains allowed roots from durable workspace settings, joins the current media index by root/path/modified time before result limits, and uses the existing pinned model/pipeline keys. The attached media index is opened read-only through a fixed app-data path; request data cannot supply database paths. Cached lookup requires no model inference or People regrouping. Meaning search validates the pinned local bundle using the existing inference path, creates one text vector for all workspace roots, then ranks compatible image vectors globally. Model verification and local AI opt-in behavior are preserved.

## Bounds and limitations

Native scans are bounded to 50,000 compatible rows per provider, with at most 4,096 OCR/People matches and 200 image-content candidates. Partial results are disclosed. Text snippets are bounded, while matched terms are retained separately so snippet truncation does not discard legitimate matches. Metadata search remains over the complete currently loaded Library snapshot. New full-text/vector acceleration and large-result performance work belong in Phase G.

Tags currently mean Open-Photo-managed XMP keywords; arbitrary foreign XMP namespaces are not newly imported by this phase. Ratings/favorites use existing local metadata. OCR remains English-first. People search reads existing named identities without generating or regrouping faces. Unindexed, changed or incompatible model-version entries cannot produce current indexed matches. Freshness uses the existing index's modified-time contract, not an export-grade content fingerprint.

The result contract accepts video-content evidence only with valid video timestamps. Phase F must supply bounded representative-frame embeddings and those timestamps; Phase E does not yet index the middle of a video or claim video semantic matches. Installed WebView2/hardware search acceptance remains distinct from native SQLite/provider tests and browser fixtures.
