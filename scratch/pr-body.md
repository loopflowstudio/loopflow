## Try it!

Inspect the explicit agent-bus namespace:

```bash
cargo run -q -p loopflow --bin lf -- radio --help
```

In one terminal, subscribe:

```bash
cargo run -q -p loopflow --bin lf -- radio sub product
```

In another, publish:

```bash
cargo run -q -p loopflow --bin lf -- radio pub \
  -c product --from demo "button audit finished"
```

The subscriber prints `[product] demo: button audit finished`. Bare `lf radio`
shows `pub` and `sub`; `lf radio "text"` and `lf sub product` fail with parser
guidance.

Exercise the Linear-backed read model without writing Linear or SQLite:

```bash
cargo run -q -p loopflow --bin lf -- pm sync --wave product --plan
cargo run -q -p loopflow --bin lf -- pm show --wave product --json \
  | jq '{wave, synced_at, projects: [.projects[] | {slug, definition, krs}], items}'
```

Run the complete repository gate:

```bash
uv run python scripts/test.py --all
```

All six suites pass: 53 Python tests; Rust format and clippy; 59 website tests
with 3 skips; 301 Swift tests plus boundary checks; CLI e2e smoke; and signed
macOS `build-for-testing`. The final changed-aware rerun passed all 1,338 Rust
tests plus the website and Swift suites after Project archival was added.

## Intent

Give machine communication and planning one explicit owner each. The agent bus
is `lf radio pub/sub`; the human thread remains `lf chat`. Linear owns Projects,
KRs, and Issues; SQLite is the daemonless local read model consumed by the CLI,
agents, and Mac. The change removes repository planning mirrors and keeps every
surface on the same typed export.

## Assumptions

- Removing the old radio and file-based planning forms is intentional; internal
  CLI and config compatibility is not preserved without an explicit migration.
- Linear Project content can hold the full definition and KR markdown, while
  its description remains a short summary.
- Planning reads may be stale between explicit syncs. They must never call
  Linear implicitly.
- Linear's current GraphQL schema uses `String!` for the Initiative, Project,
  Issue, workflow-state, and team identifiers used here.
- Resident cron expressions use UTC; product's daily wave flow runs at 08:00
  UTC.

## Key decisions

- Use nested Clap subcommands and reserve retired `sub` as a clear parser error.
- Store one atomic JSON snapshot per `(repo, wave)` instead of normalizing a
  second planning database or regenerating markdown.
- Serialize `PmShowResult` directly so Rust operations, CLI JSON, and Swift
  decode one shape.
- Refresh the affected snapshot after every PM mutation.
- Archive promoted or retired Projects through Linear before refreshing the
  source wave snapshot.
- Render the full Project definition in the Mac and use native Project
  association for backlog grouping.

## Not included

- Compatibility aliases or migration shims for the retired radio and project
  file surfaces.
- Changes to bus delivery semantics or `lf chat`.
- Background Linear refresh, remote Mac plan queries, task-history trends, or
  timezone-aware scheduling.
