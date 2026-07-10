## Try it!

Start a wave and leave it running:

```bash
lf loop product
```

From another terminal, steer the body now playing, enqueue a flow at its
current scope, and skip a step:

```bash
lf chat --wave product --steer "prioritize the recovery path"
lf enqueue review-design --wave product
lf skip --wave product
```

Open the `product` wave in Loopflow Mac. The same thread shows the current
flow/step breadcrumb, body boundaries, queued work and return point; the other
panes show its KRs, open/draft PRs, active sessions, and filed backlog. Stop and
restart `lf loop product`: journal replay restores the thread and playhead, and
an abandoned body is marked terminal without losing the logical step.

For a fast code-level check:

```bash
cargo test -p loopflow wave::runtime::tests::force_finalize_closes_the_turn_and_drops_late_deltas
cargo test -p loopflow lf::commands::waves::tests
swift test --package-path swift -Xswiftc -gnone --filter RegistryQueryTests
```

## Intent

Make a wave feel like one continuous mind even though its execution bodies are
disposable. The wave keeps one journal-backed human thread and a durable
playhead, can inhabit or delegate work with the same loop primitive, accepts
live steering where the harness supports it, and exposes the same model through
the CLI and Mac app.

## Assumptions

- A wave has one resident writer and the journal is authoritative for recovery.
- Worktree placement says where a body acts; it does not create another mind.
- Child loops inherit memory and recent wave context but keep private
  transcripts unless promoted to a wave.
- Codex supports live steering. Claude and OpenCode currently receive steering
  at the next body boundary.
- Live PR synchronization populates the local store; `lf status` does not query
  GitHub during Mac polling.

## Key decisions

- Keep sessions disposable and continuity in the journal rather than preserving
  a vendor transcript as product state.
- Model nested execution as invocation frames with local FIFO continuation
  queues, so an inserted flow finishes before returning to its caller.
- Make `--detach` the explicit concurrency switch; foreground and detached work
  otherwise use the same `lf loop` contract.
- Reserve new conversation surfaces for project promotion, when a project truly
  earns its own residency, cadence, budget, and thread.
- Carry explicit optional PR fields across the Rust/Swift status wire so missing
  data is visible and drafts remain distinct from open PRs.

## Not included

- The channels-as-topics bus/thread rewrite and server-stamped attribution.
- True mid-turn steering for non-Codex harnesses.
- First-class composite playhead nodes.
- PM label removal during promotion or new project-loop default caps.
- A persisted foreground/background run label or a one-writer worktree lock.
