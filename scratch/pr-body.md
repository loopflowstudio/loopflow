## Try it!

Inspect the formal lifecycle without creating external state:

```bash
cargo run -q -p loopflow --bin lf -- task --help
cargo run -q -p loopflow --bin lf -- status infrastructure --json |
  jq '{wave: .wave.name, active_tasks: .wave.active_tasks, tasks}'
```

With a Linear-linked Wave and an existing open task:

```bash
lf pm sync --wave infrastructure
lf task run INF-123 --json
lf task status INF-123 --json
lf task steer INF-123 "also audit the parser"
lf task wait INF-123 --until submitted
```

`run` returns after one durable Task Session and sibling worktree are registered
and the provider is running. `status` reports the same Linear UUID, human issue
identifier, session, worktree, process generation, provider thread, PR, and
latest event.

Run the repository gate:

```bash
uv run python scripts/test.py --all
```

All six suites pass. The implementation diff is net-negative outside
`scratch/`: 7,454 additions and 7,676 deletions.

## Intent

Make Wave, Project, and Task the one operating model. A Wave stays permanently
steerable in its home; Linear owns Projects/KRs/tasks; every concrete change
runs in one durable Task Session with an immutable worktree, resumable provider
history, structured commands/events, and one PR to `main`. Removing generic
loops, stacks, queues, rotation, and exec proxies leaves one answer for how work
starts and who owns it through review and merge.

## Assumptions

- Existing work has a Linear identity before execution. A bounded-fresh PM
  snapshot may launch known work while Linear is temporarily unreachable.
- Task workers are trusted local processes. Worktrees isolate writers; they are
  not security sandboxes.
- tmux, the configured provider CLI, git, and `gh` are available on the machine
  running a Task Session.
- Independent Task PRs target `main`; dependency and integration delivery are
  separate future capabilities.
- A submitted PR is not task completion. The same session remains responsible
  for review, CI repair, merge observation, and Linear writeback.

## Key decisions

- Persist Task Session identity, process generations, commands, and events in
  SQLite; keep provider thread ids and tmux names as resumable runtime state.
- Reserve Wave capacity transactionally before both initial launch and resume.
- Preserve committed Linear mutation outcomes when snapshot refresh fails, then
  reconcile before retrying completion so ambiguous writes do not duplicate
  tasks or PR comments.
- Use Harness capabilities for live input and queue unsupported input honestly.
- Settle provider turn boundaries transactionally: either claim queued input or
  make the Task Session inactive, never both.
- Deliver consequential Task events to the Wave as typed, idempotent journal
  observations; keep raw provider chatter in the Task transcript.
- Feed Swift from `lf --json` and the shared PM snapshot instead of adding a
  second lifecycle API.
- Delete the old public lifecycle in the same change; no compatibility aliases.

## Not included

- Multi-task integration PRs, task dependency edges, or remote execution.
- Rich Task transcript and direct steering UI.
- Durable Project Sessions and Task→Project supervision. `lf project run`
  remains a Wave directive in this PR.
- A durable observation outbox/cursor. The interim Wave observer is correct by
  journal idempotency but rescans Task event history every 10 seconds.
- The side-effecting live Linear/provider/PR dogfood and the complete
  10-scenario × 3-adapter scripted-peer matrix. Typed Task observations,
  decisions, atomic turn settlement, waitable receipts, and provider capability
  profiles are implemented and pass the normal Rust gate; keep the PR draft
  until the remaining parity evidence is captured.
