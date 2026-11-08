# Workflow 1427 continuation review

Reviewed November 5, 2026 against development baseline
`0055601f0650cd74f9ad67bbda62c0c43bf5e69f` and the two pending PR patches.

| Work | Exact head | Verified status | Next action |
| --- | --- | --- | --- |
| PR #115, Windows sharing experiment | c249640bbd301fbcd073f51bd90bc0e25489f5ca | CI #1425 frontend, Rust and short model acceptance passed; installer skipped | Strengthen replacement assertions and retain successful test output before relying on the experiment |
| PR #116, writer-first sharing | 60dfad6425f227a8bfdc3d34d7823ef12a10b669 | CI #1427 frontend, formatting and Clippy passed; native tests running at review | Verify completed exact-head native checks before integration |

CI #1427 subsequently completed successfully. Both pending PRs have passing
CI on their recorded heads. Follow-up PR #117, commit
`64c063cd8df9cb1bc819f1d2ecdca120263f819d`, passed local Windows verification:
274 native tests passed, eight ignored, Clippy and formatting passed. The
symlink fixture was unavailable because Windows denied privilege 1314.
PR #117 now targets development and CI #1428 is running:
https://github.com/MieleMadness/Open-Photo/actions/runs/42143641778
Its initial stacked target did not match the workflow's PR branch filters;
retargeting and reopening started verification. Coordinate #115/#117 integration
because #117 includes #115's experiment.

## Findings

The replacement experiment in #115 accepts either old or new destination bytes
regardless of the replacement result. It can therefore pass if the replacement
reports success but leaves old bytes. Its diagnostic is captured by the Rust
test harness and omitted for passing tests in the current workflow. Neither
green CI nor that test alone establishes a usable Windows publication protocol.

The workflow continuation adds `--show-output` to native regression execution,
retaining diagnostics from successful tests without enabling ignored tests.
The explicit ignored real-model acceptance command remains unchanged.

The successful-publication external-editor race is still an intentionally
ignored failing gate in `xmp_publish_race_tests.rs`. Production conflict safety
and crash durability remain open. Do not interpret either experimental PR as
closing Issue #90 or satisfying installed Windows acceptance.

## Independent queue

1. Strengthen #115's success/result assertions and record the actual Windows
   replacement outcome; review #116's completed result alongside it.
2. Continue recovery inspection and explicit user-review design separately
   from the production publication protocol; preserve every recovery artifact.
3. Resume Phase J crossfades only after its persistence and acceptance gates.

Keep about three independent PRs in flight, preserving all concurrent changelog
and roadmap entries at integration. This review does not authorize a release.
