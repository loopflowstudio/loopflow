# LOO-496 assumptions

The untracked `.lf/flows/repair-ship-auto.yaml` and `.lf/skills/repair-ship-auto.md`
are supplied execution instructions, not part of the hook repair. Preserve them
locally and exclude them, together with untracked scratch notes, only in the Git
environment of delivery commands. No global ignore configuration is changed.

## 2026-10-27 — thread hook trust

Loopflow again trusts its own exact Codex hook declarations, per thread, which
reverses #1531's "provider owns review". Codex 0.164 offers no review path for
thread-config hooks. The alternative, launch-level `-c` declarations that the
`/hooks` screen can see, would replace wrapper hooks on the same events and
would need re-review after every declaration change. Global trust writes and
`--dangerously-bypass-hook-trust` stay excluded. Reversible: delete the `state`
insertion in `codex_config`.
