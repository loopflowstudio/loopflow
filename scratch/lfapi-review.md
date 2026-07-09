# lf command API redesign review

## What was implemented

Promoted the mechanical `lf op ...` surface into first-class commands:
`lf pr`, `lf wt`, `lf rebase`, `lf commit`, `lf auth`, `lf release`, and
`lf pm`. Flow operation items now parse against the same human grammar, so
builtin flows use payloads like `op: pr land --create-pr` instead of
`op: land --create-pr`.

The docs, builtin skill prompts, website docs index, goldens, install scripts,
e2e smoke scripts, and release automation tests were swept to match the new
surface. The lfd exec and webhook paths now validate and emit the new argv
shape, including a regression test that webhook-planned commands do not fall
through to external subcommands.

Gate polish fixed remaining stale current-facing references in README, tmux
helper scripts, install skill sync, skill-sync verification, Python model tests,
and Rust flow parser tests.

## Key choices

- Kept `op:` as the flow-step type. The redesign removes the CLI drawer, not
  the engine's flow operation marker.
- Made bare `lf pr` show PR status, with `lf pr open`, `lf pr submit`, and
  `lf pr land` carrying the lifecycle verbs.
- Kept PR-mutating commands on the existing rebase-conflict retry path.
- Deleted the mechanical verbs the design retires: `next`, `advance`,
  `branches`, `sync`, `doctor`, `shell`, `cp`, `push`, `queue reconcile`.
  Only `sync-skills` survives, hidden (`#[command(hide = true)]`), because
  `install.py` and `verify_skill_sync.py` invoke it. Machine callers that
  needed `next` and `queue reconcile` now call the library in-process.

## How it fits together

`lf/mod.rs` defines the new top-level clap grammar. `bin/lf.rs` routes each
top-level command into the existing ops implementation functions, while
`ops/flow.rs` converts `op:` flow items into the same parsed CLI commands.
Docs and prompts were updated so humans, flows, webhooks, and remote exec all
speak the same command language.

## Risks and bottlenecks

- The old `lf op` command is removed without aliases. Any unswept external
  automation will fail fast instead of silently using the old shape.
- The design/implementation mismatch around retained top-level plumbing verbs
  needs product confirmation before merge.
- Full matrix Rust nextest passed with one test reported as "leaky"; the suite
  still exited successfully.
- Loopflow UI Xcode tests built and launched in the full matrix, then the UI
  runner sat idle at 0% CPU in this headless session. I interrupted it and
  killed the orphaned `LoopflowUITests-Runner`; rerun that check in an
  interactive macOS UI test environment if required.

## What's not included

- No backwards-compatible `lf op` shim.
- No migration layer for old flow operation payloads.
- No deletion of historical release notes that mention `lf op`; those remain
  historical records.

## Validation

- `cargo fmt --all -- --check` - pass
- `cargo clippy --all-targets -- -D warnings` - pass
- `cargo test` - pass (was red before the compress pass: two `/v0/exec` door
  tests asserted a 400 on `next --nonesuch`, but with `next` deleted that argv
  parses as an external subcommand and reaches exec instead of being refused)
- `uv run python scripts/test.py --all` - Python pass; Rust pass; website pass;
  Swift package pass; e2e smoke pass; Loopflow UI built and then hung idle in
  headless UI test execution as noted above
