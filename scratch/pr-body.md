## Try it!

```bash
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

What you should see:
- Rust coverage now includes `journal::tests::journal_uses_configured_run_id_when_present` and `invalid_configured_run_id_falls_back_to_generated_id`, which prove `LF_RUN_ID` is honored when valid and ignored when invalid.
- The daemon journal observer still replays runtime events, but now it goes through the shared `read_events()` helper instead of its own parser.
- The final `xcodebuild test` command still fails locally here: both runs ended with `ConcertoUITests-Runner ... crashed with signal kill before establishing connection` after the package/unit tests passed.

## Intent

Make the shipped journal contract usable by the future real-CLI executor. The daemon already has a canonical run id and already knows how to replay CLI journal files; this branch closes the gap by letting the CLI journal adopt a daemon-supplied `LF_RUN_ID`, while moving the larger shared-`FlowEngine` executor redesign into `scratch/` so the wave roadmap only tracks work that is still left to build.

## Assumptions

- Future daemon-launched CLI runs will inject a valid `LF_RUN_ID`.
- Invalid or missing `LF_RUN_ID` values should never block journal emission; falling back to a generated id is safer than failing the run.
- The CLI and daemon should share one journal parser whenever possible.

## Key decisions

- Parse `LF_RUN_ID` as an `LfdId` and ignore invalid values with debug logging.
- Reuse `journal::read_events()` inside `LfObserver` rather than keeping a second JSONL decoding path.
- Delete `wave/lfd/01-real-cli-executor.md` and keep the unfinished executor plan in `scratch/lfd-real-cli-executor.md` until implementation starts.

## Not included

- The shared `FlowEngine` / daemon executor refactor itself.
- Daemon-side environment injection for `LF_RUN_ID`, `LFD_RUN_ID`, `LFD_WAVE_ID`, or `LFD_SESSION_ID`.
- A fix for the local `ConcertoUITests-Runner` bootstrap crash.
