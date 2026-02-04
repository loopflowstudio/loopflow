# Review: Rust prompt parity + ops workflow polish

## What was implemented
- Added the `loopflow-ops` crate and switched `lf ops` to use workflow orchestration (commit/pr/land/next/abandon/rebase) with shared progress reporting.
- Implemented `--dry-run` in Rust and Python step/inline/flow commands to print prompts without launching agents.
- Enforced scheduler slot acquisition for cron/watch triggers and interactive resumes; added slot-aware run spawning and scheduler tests.
- Embedded `LOOPFLOW.md` in Rust prompt assembly for parity with Python and added prompt parity tests/fixtures.

## Key choices
- Centralize ops workflows in `loopflow-ops` so CLI and daemon share behavior while keeping `lf` thin.
- Acquire scheduler slots for every run start path (loop, watch, cron, resume) to make capacity enforcement consistent.
- Use `include_str!` for `LOOPFLOW.md` to avoid runtime file I/O while matching Python prompt content.
- Treat `--dry-run` as a fast path that avoids runner availability checks and agent startup.

## How it fits together
`lf ops` now delegates to `loopflow-ops` workflows, which wrap `loopflow-engine` primitives and surface progress via a `Progress` trait. Prompt parity is validated by Python tests that run both implementations and compare normalized outputs. The scheduler now guards all run starts, with slots released after execution completes.

## Risks and bottlenecks
- Slot enforcement can defer watch/cron activations and resume requests under load; errors surface as resource exhaustion.
- `LOOPFLOW.md` is compiled into the Rust binary; updates to the file require rebuilds to take effect.
- Parity tests rely on local Rust builds and may be slower on CI without cached artifacts.

## What's not included
- Wave-based branch naming/metadata updates for `lf ops next`.
- Remote WaveService wiring for TokenProvider auth.
- Extended parity coverage beyond the initial prompt fixtures.
