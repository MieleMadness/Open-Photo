# Review of the 0.56.10 update

Reviewed base: `50919387dc8371153c46720f2375eb17545df118`, including the 29 commits after 0.56.8. The newer page headings, timeline command hierarchy, destructive-reference styling, tooltips and status announcements are retained. They do not require a broader UI rewrite.

| Priority | Finding | Phase E response |
| --- | --- | --- |
| High | Legacy OCR query applied its limit before filtering workspace roots, allowing unrelated workspaces to crowd out current results. LIKE escaping also omitted literal backslashes. | Scope in SQL before the limit; escape backslashes/wildcards. Regression tests use competing workspace records and literal path-like text. |
| Medium | Filename, semantic and OCR queries had separate UI/result paths. Users could not get one relevance order or understand combined matches. | One parser, ranked result model and Library search; retain indexing controls and expose match reasons. |
| Medium | Semantic text inference was repeated for each folder, then separately limited per root. | One query vector and workspace-wide ranking, with visible bounded-provider limits. |
| Medium | Workflow name remained at 0.56.8 while package manifests were 0.56.10. New milestone notes contained literal escaped newline sequences. | Synchronize the next patch's version/workflow and repair the document formatting. |

Still separate work: video semantic sampling (F), list-mode virtualization and broader large-library performance (G), map clustering/metadata coverage (H), and mixed-media/social editor extensions (I). The earlier local photo-timeline work is preserved on `work/mixed-media-snapshot`; it is not silently mixed into this Phase E search patch. No source media was modified by this review or search work.
