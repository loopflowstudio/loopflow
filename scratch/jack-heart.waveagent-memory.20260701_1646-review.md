# Wave memory surface gate review

## What was implemented

Added the two-file authored wave surface: `wave/<name>/GOAL.md` is now the
wave-local goal file, and `wave/<name>/MEMORY.md` carries curated continuity for
the wave. Lowercase `goal.md` is intentionally ignored; the repo's own
`wave/goals` directory has been migrated to `GOAL.md` plus `MEMORY.md`.

Wave goal rendering now includes memory in a dedicated `<lf:wave-memory>` block.
The wave executor reads `MEMORY.md` from the run worktree when it builds the
inline goal prompt. Missing memory is normal and renders as an empty-memory
placeholder; unreadable memory is now a run-command error instead of silently
dropping context.

Wave creation and wave goal config updates initialize `MEMORY.md` without
overwriting an existing file. PM tooling, reset scripts, tests, and docs now use
`GOAL.md` frontmatter instead of the retired wave YAML/lowercase goal surface.

## Key choices

- `GOAL.md` is case-sensitive at lookup time. A lowercase `goal.md` no longer
  resolves, even on case-insensitive filesystems.
- `MEMORY.md` stays file-backed and render-time only. It is not a DB column and
  not a DTO field.
- Missing `MEMORY.md` is not an error so older waves can run. A present but
  unreadable memory path fails loudly because that is operationally different
  from "no memory yet."
- New wave memory starter text is small and bounded: one H1 plus guidance to
  summarize and prune instead of appending indefinitely.
- Public docs now teach `GOAL.md` and `MEMORY.md`; lfd-only crons/triggers are
  no longer shown as repo-authored wave YAML.

## How it fits together

`load_goal` resolves `wave/<name>/GOAL.md` before repo/user/builtin goals.
`build_wave_run_command` loads the goal, reads `wave/<name>/MEMORY.md`, builds a
`GoalRenderContext`, and launches the rendered prompt through the existing
inline `lf : ...` path.

`read_wave_config` parses `GOAL.md` frontmatter for wave intent during create
and update flows. `ensure_wave_memory` owns the sibling memory file creation and
is called from wave creation plus goal config writes.

## Risks and bottlenecks

- Goal prompts and memory still travel through the inline CLI prompt path. Very
  large memory files could eventually hit argv-size or prompt-size pressure.
- `GOAL.md` still stores some live agent override fields until runtime state is
  fully separated from repo-authored intent.
- Existing waves without `MEMORY.md` run with a placeholder; they only gain real
  continuity after memory is created or curated.
- Concerto Xcode UI tests were not run because this headless gate has no
  rendering environment. Swift package tests cover model/service parsing, not
  the full Xcode UI runner.

## What's not included

- No lfq dispatch-through-lfd or hosted looping-agent session launcher.
- No Asana live-roadmap read/write integration beyond existing PM fields.
- No memory curation/compression policy beyond file creation and injection.
- No DB or wire DTO field for memory.
- No deletion of remaining wave runtime fields such as `area`, `direction`, or
  `primary_flow`.

## Validation

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo test --all` passed: 970 Rust library tests plus binary/integration/doc
  tests, with 2 ignored.
- `uv run pytest python/tests/` passed: 157 tests.
- `cd website && uv run python dev.py test` passed: 61 tests, 3 skipped.
- `swift test --package-path swift` passed: 5 XCTest tests and 336 Swift Testing
  tests.
- `tests/e2e/test_smoke.sh` passed.
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
  passed: 16 tests.
- `uv run pytest tests/regression/ -v` passed: 4 tests.
- `docker version` passed.
- `cargo test -p loopflow docker_` passed: 11 docker-filtered tests.
- Focused rerun after memory-read tightening:
  `cargo test -p loopflow lfd::executor::wave::tests::build_wave_run_command`
  passed: 2 tests.
- Concerto Xcode UI test was skipped because this run has no rendering
  environment.
