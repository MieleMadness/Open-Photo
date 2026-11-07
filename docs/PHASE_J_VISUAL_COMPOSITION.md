# Phase J phase 1 — composed visual prototype

The test-only Windows decoder now feeds a typed visual compositor. It uses the verified seek's complementary rational weights, retains decoder/source ownership through composition and requires final consumer eligibility checks. It supports a single normalized video/photo frame, two-frame RGB dissolves and the existing discrete dip envelope. Output remains bounded to the decoder's 960×540 RGB geometry. Input buffers, temporary gain buffers and output are bounded by that geometry; composition checks cancellation between parts and before final completion.

`compose_next` retains the request's worker slot throughout decoding and composition. The common completion gate rejects superseded errors/results and revalidates native project/session bindings. `visual_current` checks again at consumer handoff. Source read pins remain held until the typed result is dropped. No source writes, production migration, IPC command, UI activation or export routing were added.

The color policy is explicit RGB-channel interpolation with rounded integer output and sample-centered dissolve weights. Bundled FFmpeg's RGB blend reference is compared with a one-channel-level rounding tolerance. The older experimental YUV xfade export graph is not silently treated as equivalent: reconciling actual production preview/export policy remains phase 3 work. Global titles and PCM audio remain phase 2 work.

Evidence: `../../audit-output/phase-j-compose-full.log`, `phase-j-compose-clippy.log`, plus the existing decoded-source regressions. The initial filtered invocation matched zero tests and is not acceptance evidence; the full suite is the verification run. The existing 0.56.86 installer predates this test-only increment and was not rebuilt or installed.
