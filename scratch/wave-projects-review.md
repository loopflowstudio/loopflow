# Wave Projects Gate Review

## What Was Implemented

This branch moves Loopflow's planning model to three nouns: waves, projects, and tasks. Local waves now hold project definitions under `wave/<wave>/projects/*.md`; tasks live in Linear and are associated with local projects through `project:<slug>` labels.

`lf op pm` now exposes that model:

- `status` summarizes linked waves, Linear project names, unassigned tasks, and open task counts by local project.
- `show --project <slug>` filters a wave's Linear tasks to one local project.
- `task create/update/done/move` provides explicit task operations while preserving `update` compatibility.
- `rename` renames the Linear project backing a wave.
- `sync --plan` reports low-risk drift: renamed Linear projects, stranded projects, missing labels, unassigned tasks, and project labels that no longer match local project docs.

Docs, built-in skills, and prompt goldens were updated so future agents keep tasks in Linear and keep project KRs as proof-shaped end states rather than backlog mirrors.

## Key Choices

- One Linear project still backs each wave. Local Loopflow projects are labels, not nested Linear projects.
- Project associations use visible Linear labels named `project:<slug>` because they are easy to inspect and can be migrated incrementally.
- `lf op pm update` remains as a compatibility path, but the documented path is the explicit `task` subcommand family.
- `sync --plan` diagnoses ambiguous drift instead of guessing task moves.
- Old local roadmap mirrors were removed in favor of `GOAL.md`, `MEMORY.md`, local project docs, and Linear tasks.

## How It Fits Together

`wave/<wave>/GOAL.md` pins the wave's backing Linear project through `pm.linear_project`. `rust/loopflow/src/ops/pm.rs` resolves that wave context, talks to the Linear client, and maps local project slugs to `project:<slug>` issue labels. CLI output in `rust/loopflow/src/lf/commands/ops/mod.rs` groups and filters tasks using those labels.

The prompt layer mirrors the same contract: waves own operating context, projects own KRs, and tasks live in Linear.

## Risks And Bottlenecks

- Live Linear mutations were not executed during gate because they would rename projects, create labels, move issues, or close tasks in the real workspace. Rust tests cover the GraphQL request shape and local filtering behavior.
- `sync --plan` currently assumes the configured provider set is effectively Linear. That matches the branch scope but is not a general multi-provider reconciliation layer.
- Local `xcodebuild test` for the Loopflow UI suite fails in this headless run because `LoopflowUITests-Runner` hangs before establishing its connection. The app/unit test bundle inside that command reports 304 passing tests before the runner failure.

## What's Not Included

- Automatic migration of ambiguous existing Linear tasks into local projects.
- Removing stale labels from tasks when moving between local projects.
- A general PM provider abstraction beyond the current Linear implementation.
- Any Swift/UI behavior change.

## Validation

- `git diff --check main...HEAD`: pass.
- `cargo fmt --check`: pass.
- `cargo clippy -- -D warnings`: pass.
- `cargo test -p loopflow golden_prompt`: pass after regenerating prompt goldens.
- `uv run python scripts/test.py`: pass for changed suites: Rust and website.
- `uv run python scripts/test.py --all`: Python, Rust, website, Swift package, and e2e smoke passed; Loopflow UI failed with the local `LoopflowUITests-Runner` connection hang described above.
- Rerun of `xcodebuild test -project LoopflowSwift.xcodeproj -scheme LoopflowMac -destination 'platform=macOS' -derivedDataPath /tmp/loopflow-xcode-gate CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`: reproduced the same UI runner connection hang after 304 app tests passed.
