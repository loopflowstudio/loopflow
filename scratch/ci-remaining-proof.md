# Remaining CI proof — October 13, 2026

PR #1543 merged; the full five-minute goal remains unmet. Latest real integrated changes contradict stable sub-five-minute performance. Exclude warmed reruns, scratch checkpoints and exact-proof-only main pushes from the ten consecutive typical-code PR sample.

| Run | Event | Creation to aggregate completion | Limiting evidence |
|---|---|---|---|
| [38447800008](https://github.com/loopflowstudio/loopflow/actions/runs/38447800008) | pull_request | 5:22 | rust-test (integration, 2, 3): 5:16; swift-test: 4:15; loopflow-ui-test: 3:33 |
| [38448561480](https://github.com/loopflowstudio/loopflow/actions/runs/38448561480) | merge_group | 5:44 | rust-test (integration, 2, 3): 5:24; swift-test: 5:23; loopflow-ui-test: 5:18 |
| [38452715134](https://github.com/loopflowstudio/loopflow/actions/runs/38452715134) | merge_group | 6:24 | rust-test (integration, 2, 3): 4:44; swift-test: 5:29; loopflow-ui-test: 6:06 |
| [38453608016](https://github.com/loopflowstudio/loopflow/actions/runs/38453608016) | push | 6:06 | swift-test: 5:23; loopflow-ui-test: 5:59 |

Latest queue 38452715134 restored exact dependency caches for both Swift CLI and app control CLI. Xcode DerivedData missed because Swift source inputs changed. App control compilation reported 1m33s within the serial Xcode app/post-build/test-bundle sequence. Xcode compile step: 271s. Swift transport CLI:69s followed by Desktop/test step154s. Rust build/archive:81s; all Rust partitions finished before five minutes. Aggregate scheduling also consumed measurable margin; do not attribute the full elapsed delta to compilation.

lf-ddf2d53 owns the [macOS critical-path change](macos-ci.md), separately from local-speed/disk lf-8f870b1 and test deletion lf-6d3ff35. lf-12be237 retains aggregate ten-run and queue/push-to-merged proof. The earlier aggregate-target experiment also remains rejected.

Sources: GitHub Actions run/jobs APIs and log for run 38452715134, inspected October 13. Exact push timestamps are still missing; publication start is only an upper bound.
