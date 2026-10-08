The harness selector is now `--agent` / `-a`, including Flow and Session child launches. Interactive work stays in the terminal: the removed `--ide` path could report success while leaving a Session without the native identity needed to reconnect.

## What changes

- Replace Loopflow’s `--model` / `-m` selector with `--agent` / `-a`; preserve native provider model flags and the `agent` configuration key.
- Delete app URL launching, `session.launch`, automatic skill export during app launch, shortened skill prompts and the handoff writer. Preserve terminal process receipts and historical records.
- Retain reproducible Claude/Codex skill transport probes and document their evidence limits and remaining design.

Unified skill discovery, native skill dispatch and translated ports remain unfinished. This PR does not claim that arbitrary third-party skills already run through Loopflow.

## Checks

After merging current main: architecture checks, Rust formatting, Clippy and Ruff pass; 76 website tests pass. Python/probe tests passed 414 cases under network isolation; the macOS sandbox fixture passed separately using its own network-denying policy because macOS refused nested sandbox application.

The materialized Rust source-copy suite passed all 2,284 runnable tests; 17 installation/platform cases were skipped. The initial unmaterialized run failed on the wrong schema and was stopped; only the corrected run establishes the Rust result. No installation or live-provider fidelity acceptance is claimed.
