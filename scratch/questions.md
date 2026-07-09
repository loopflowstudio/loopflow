# Unresolved scope

- The live wave pass still runs as a captured one-shot `lf -b flow wave`
  process. Inbox messages journal immediately and queue at the pass boundary;
  they are not yet injected into the running Codex harness at tool boundaries,
  and assistant text journals when the pass exits rather than incrementally.
  Closing this requires moving `flowloop/wave.rs` onto the existing session
  harness API instead of adding stdin control to execs. Detached loops are
  deliberately headless and tmux is documented/read back as `attach -r`, so
  the old unjournaled mutation door is closed meanwhile.
- Project promotion can create the child PM project and move its labeled tasks,
  but the PM provider abstraction has no remove-label operation. The promotion
  skill records residual `project:<slug>` labels here when it cannot remove
  them safely; adding provider-level label removal is separate API work.
- Residency reads wave definitions from the main checkout. When promotion is
  authored in a worker worktree, the command now stops with an explicit
  land-before-residency error instead of launching a child against files the
  listener cannot see. Fully automating that handoff would make project
  promotion own the review/merge policy, which is outside this command.
- `lf loop project` still inherits the generic 8-pass / 2-hour caps. The new
  command exposes explicit cap overrides and the doctrine requires durable
  reports/memory, but selecting longer project defaults needs real dogfood data
  rather than a guessed weeks-scale timeout.
- The run ledger now exposes a loop's current pass, but it does not
  persist whether the owner was the foreground wave pass or the listener's
  detached tmux supervisor. The Mac screen therefore shows pass/worktree and
  liveness without inventing an unreliable foreground/background label.
- The broader bus/thread rewrite from `scratch/minds.md` remains separate from
  this Changes implementation: channel addressing still carries the existing
  client attribution and `lf chat --parent` transport. This branch implements
  the requested execution, memory, promotion, control, backlog, doctrine, and
  Mac surfaces without silently changing that wire protocol.
