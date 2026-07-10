## Try it!

Start one persistent wave mind:

```bash
lf serve product
```

From another terminal, steer the body now playing, enqueue a flow at its
current scope, and skip a step:

```bash
lf chat --wave product --steer "prioritize the recovery path"
lf enqueue review-design --wave product
lf skip --wave product
```

Watch the agent bus without depending on the served wave, then publish a report
from another shell:

```bash
# terminal 2
lf sub product

# terminal 3
lf radio -c product.demo --from demo "recovery check passed"
```

The subscriber prints the row immediately; the served mind folds the family
report into its journal-backed thread. Stop and restart `lf serve product`:
the thread and playhead replay, and an abandoned body is closed without losing
the logical step. In Loopflow Mac, the same wave screen shows the thread,
flow/step breadcrumb, body boundaries, KRs, open/draft PRs, active sessions,
and filed backlog.

For a fast code-level check:

```bash
cargo test -p loopflow wave::bus::tests
cargo test -p loopflow flowloop::driver::tests
swift test --package-path swift -Xswiftc -gnone --filter RegistryQueryTests
```

## Intent

Make a wave feel like one continuous mind even though its execution bodies are
disposable. The wave keeps one journal-backed human thread and durable
playhead, inhabits or delegates through the same loop primitive, accepts live
steering where the harness supports it, and receives agent reports through a
store-backed bus that works while no wave process is running.

## Assumptions

- A wave has one resident writer and the journal is authoritative for thread
  and playhead recovery.
- Worktree placement says where a body acts; it does not create another mind.
- The local SQLite store exists when agents need the bus or registry. With no
  store, publish prints a drop note and exits 0.
- Bus messages need only a one-hour handoff window; PRs and `lf runs` retain
  durable work evidence.
- Bus bylines are client testimony recorded beside the arrival channel, not an
  authentication boundary.
- Codex supports live steering. Claude and OpenCode currently receive steering
  at the next body boundary.

## Key decisions

- Keep sessions disposable and continuity in the journal rather than
  preserving a vendor transcript as product state.
- Model nested execution as invocation frames with local FIFO continuation
  queues, so inserted flows finish before returning to their caller.
- Make `--detach` the explicit concurrency switch; foreground and detached
  work otherwise use the same `lf loop` contract.
- Separate human thread and agent bus by both verb and transport: `lf chat` is
  durable HTTP/SSE on a served mind; `lf radio`/`lf sub` use the shared store.
- Prefer at-least-once report delivery: journal before cursor commit, expose
  expired gaps, and keep PR/run records as the durable fallback.
- Reserve new conversation surfaces for project promotion, when a project
  earns its own residency, cadence, budget, and thread.

## Not included

- True mid-turn steering for non-Codex harnesses.
- First-class composite playhead nodes.
- PM label removal during promotion or new project-loop default caps.
- A persisted foreground/background run label.
- A detached hand's private bus cursor or a one-writer worktree lock.
- Consolidation of the overlapping `lf chat` and `lf wavechat` surfaces.
