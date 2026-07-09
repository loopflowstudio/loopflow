# Unresolved scope

The built/not-built ledger lives in `scratch/minds.md` under **Status**. This
file carries only the judgment calls a reader would otherwise re-litigate.

- Skill steps run in the live harness and Codex steering journals and streams
  incrementally. Claude and OpenCode expose no true mid-turn steer capability,
  so their input queues to the next body. Composite top-level flow nodes still
  run through the internal headless step fallback; promoting branch/loop
  internals into first-class playhead paths is separate graph-runtime work.
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
- Two agents wrote this worktree at once. HEAD advanced under a running skill
  while unrelated files were being edited, and the two writers left a
  self-contradicting test behind — one updated the frame it waited for, the
  other left the assertion on the old shape. Nothing enforces one writer per
  worktree. Whether that is a wave-home invariant or a lock is open; until it
  is settled, check for a live agent before working a wave worktree.
