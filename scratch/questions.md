# W2-135 PR2 assumptions

- A `--model` handoff rejects a body whose Session still has a live writer. The
  operator can interrupt it first; PR2 does not add a force-kill path.
- Provider transcript handles are compatible only within the same harness
  family. `claude` to `claude:opus` retains the handle; `claude` to `codex`
  clears it before the next generation is reserved.
- Passing the already-selected agent is a normal resume, not an audited
  handoff. It preserves the existing provider history.
- PR2 opens the replaceable-body recovery path but does not by itself prove the
  broader W2-135 supervision finish line, so this PR must leave W2-135 open.
