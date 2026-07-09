## Try it!

```bash
lf op pm status
lf op pm show --wave product
lf op pm show --wave product --project wave-chat
lf op pm task create --wave product --project wave-chat --title "Retain steward thread after restart"
lf op pm task move --id <task-id> --wave product --project loopflow-api
lf op pm rename --wave product --title "Product"
lf op pm sync --plan
```

Validation run:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test -p loopflow golden_prompt
uv run python scripts/test.py
uv run python scripts/test.py --all
```

`scripts/test.py --all` passed Python, Rust, website, Swift package, and e2e smoke. The Loopflow UI suite is blocked locally by `LoopflowUITests-Runner` hanging before it connects; the app/unit test bundle inside that command reports 304 passing tests before Xcode returns exit 65.

## Intent

Make Loopflow's planning model explicit in code and docs: waves are durable operating contexts, projects are measured bets under one wave, and tasks live in Linear. `lf op pm` can now group, filter, create, move, close, diagnose, and rename Linear tasks/projects in terms of the local wave project tree.

## Assumptions

Each wave has at most one backing Linear project in `wave/<wave>/GOAL.md`. Local projects are markdown files under `wave/<wave>/projects/`. Linear labels named `project:<slug>` are the visible association between tasks and local projects.

Live Linear mutation demos were not run in gate because they would create labels, move issues, close tasks, or rename real Linear projects.

## Key Decisions

One Linear project remains the task container for each wave; local Loopflow projects are represented as labels instead of additional Linear projects. `lf op pm update` remains compatible, but the documented API is now `lf op pm task ...`. `sync --plan` reports ambiguous drift instead of guessing task migrations.

## Not Included

No automatic migration of ambiguous existing Linear tasks. No stale-label removal during task moves. No PM provider beyond the current Linear path. No Swift/UI behavior changes.
