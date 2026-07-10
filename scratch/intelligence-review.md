# Context-tiered execution review

## What was implemented

Loopflow now treats the current process and worktree as the default execution
surface. The universal operating prompt teaches inline execution and mechanical
`lf` operations; wave, project, and task skills add only the orchestration
capabilities their tier owns. Waves may create project or task loops, projects
may create task loops, and tasks never invoke `lf loop`.

The CLI now normalizes unambiguous global flags across built-in and nested
subcommands while preserving local collisions and `--`. Loops reject fewer than
two passes before wave, registry, worktree, or server resolution. Foreground and
detached loops use the placed registry run id as their trace id, explicit wave
scope drives context and ledger attribution, and placed work keeps its own
`wave.<run>` bus channel.

## Key choices

- Keep the universal `LOOPFLOW.md` as a capability floor. PM discovery, server
  startup, and loop recipes live only in the skills that exercise them.
- Model delegation as a separate repeated lifecycle, not any child process.
  Direct skills, flows, tests, and mechanical `lf` commands remain inline.
- Derive flag ownership from Clap instead of maintaining another flag list.
  The selected command path decides local ownership; a local spelling wins over
  a top-level spelling.
- Reject one-pass loops instead of treating them as a degenerate placement
  mode. `lf flow` is the direct one-shot surface.
- Mint one id before placement and carry it through the registry, detached HTTP
  handoff, pass environment, prompt logs, and ledger. Each pass still mints a
  fresh process span.
- Root placed traces by clearing the caller's process id, and override the wave
  channel with the placed run's `wave.<short-run-id>` channel.

## How it fits together

Prompt assembly injects the universal execution floor, then the active tier
skill supplies its bounded orchestration policy. The CLI resolves argument
scope and explicit wave identity before dispatch; loop placement creates the
registry run, then pass launch exports that run id as `LF_RUN_ID` while removing
`LF_PROCESS_ID`. The same placed identity supplies the worker's bus channel, so
status, trace evidence, and reports describe one unit of work.

## Risks and bottlenecks

- Argument normalization is a pre-parser. Its tables come from Clap, but command
  semantics still need regression tests whenever nested flag ownership changes.
- Explicit top-level `--wave` is now an identity claim and requires a matching
  local registry row. This intentionally fails early instead of assembling one
  wave's context while attributing evidence to another.
- Detached execution still depends on an already-running wave server, tmux, and
  a valid capability token. The HTTP, argv, worktree, and identity paths are
  covered by tests; the gate did not spend an LLM run on a live detached task.
- `lf pm show --wave intelligence` currently reaches Linear but fails on a
  GraphQL `ID!` versus `String!` variable mismatch. The branch remains
  computable from its design and project docs; PM adapter repair is outside
  this change.

## What's not included

- No server auto-start, PM/auth repair, or inferred wave selection.
- No new delegation flag or tier enum in Rust; tier policy remains authored in
  skills and flows.
- No rejection yet for every command that parses but ignores `--fork` or
  `--stack`; that existing CLI gap remains separate work.
- No changes to eval harvesting, delivery records, or escalation telemetry.

## Validation and measures

- `LOOPFLOW.md`: 9,147 bytes / 187 lines before; 4,707 bytes / 104 lines after
  (48.5% fewer bytes in every default assembled prompt).
- One-pass CLI validation: bare, post-command `--wave`, and canonical pre-command
  `--wave` forms all fail with the direct `lf flow` correction before wave
  lookup.
- `uv run python scripts/test.py --all`: all six suites passed.
  - Python: 51 passed.
  - Rust: 1,314 passed, 3 skipped by suite configuration.
  - Website: 59 passed, 3 skipped by suite configuration.
  - Swift: 5 XCTest and 307 Swift Testing tests passed.
  - E2E smoke: passed.
  - Loopflow macOS test build: succeeded.
- `cargo clippy -- -D warnings`: passed.
- `cargo fmt --check`: passed.

## Wave alignment

This advances the Context project's standing KRs directly: the default prompt
is smaller, orchestration doctrine is bounded by execution tier, and placed
work now preserves trace and channel attribution instead of producing evidence
that disagrees with the registry.
