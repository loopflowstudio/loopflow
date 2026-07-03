## Try it!

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
uv run python scripts/check_swift_multiplatform_boundaries.py
cd website && uv run python dev.py test
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
uv run pytest tests/regression/ -v
docker version && cargo test -p loopflow docker_
```

Expected: Rust/Python/Swift DTO fixtures agree on `Wave`, `Run`, and `Session`;
`lfq wave run` and `lfq worker run` return durable sessions; `/v0/runs` exposes
queue projection; `/v0/sessions` exposes attachable control surfaces.

This gate passed the same checks. In this local uv environment, console scripts
for `pytest` and `playwright` did not spawn through `uv run <script>`, so the
equivalent `uv run python -m pytest ...` and `uv run python -m playwright ...`
entrypoints were used for validation.

The Concerto xcodebuild UI command was not run in this headless/no-rendering
gate:

```bash
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Use CI or a rendered macOS session for that UI runner. `swift test
--package-path swift` passed locally.

## Intent

Reduce lfd's public runtime model to the product nouns that matter: `Wave`,
`Run`, and `Session`. This removes overlapping prototype concepts, aligns Rust,
Python, Swift, DTO fixtures, docs, and Concerto around one vocabulary, and makes
`lfq` the runtime-control surface for wave agents, workers, and attachable
sessions.

## Assumptions

- This branch does not preserve compatibility aliases for removed internal API
  names or routes.
- `Conversation` remains an internal transcript/input transport detail;
  reviewers should evaluate the product/API surface as `Session`.
- Wave ancestry is intentionally deferred to `wave/goals/2-wave-ancestry.md`.

## Key Decisions

- `Run` absorbs the old `WaveRun` and `AgentRun` execution/result lineage.
- `Session` absorbs the old terminal session and conversation session surfaces,
  with `use = wave_agent | worker | palette`.
- Launch routes return durable sessions; attach info comes from session attach
  endpoints instead of launch envelopes.
- Queue state moved with the `Run` DTO and docs now point to `/v0/runs`.

## Not Included

- Durable `Wave.parent_wave_id` restoration or populated child-wave trees.
- Backwards-compatible `/v0/wave_runs`, `/v0/terminal-sessions`,
  `wave_run_id`, or `terminal_session_id` shims.
- A hosted replacement for Codex/Claude cloud runtimes.
