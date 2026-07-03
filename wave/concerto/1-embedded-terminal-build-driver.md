---
asana_id: '1214269992004911'
---
# Embedded terminal build driver

**Finish line:** Concerto's embedded terminal is the daily driver for wave work.
Beyond the `/goal` launch that already ships, the palette launches arbitrary
flows/steps into embedded tmux panes with the right worktree, sessions reattach
across restarts, per-wave layout survives, and lfd-backed live status can see the
sessions `lf` launches. Dropping to an external Ghostty window is a deliberate
choice, not the default.

## What shipped (this branch)

The launch/attach spine is in: `lf goal <wave> --tmux`
(`rust/loopflow/src/lf/commands/goal.rs`, `launch_in_tmux`) creates-or-reuses the
wave worktree `../<repo>.<wave>`, runs the goal loop in a detached tmux session
named `lf-<repo>-<wave>`, prints the handle, and is idempotent. Concerto's
`WavesView` click shells out to the bundled `lf`, reads the handle, and attaches
via the existing `tmux attach-session` / `GhosttyTerminalView` path — **no lfd in
the launch/attach path**. Running state is derived from `tmux has-session`.

## What's left to make it the daily driver

- **Session registry so live status sees `lf`-launched sessions.** `lf goal
  --tmux` does not register its session anywhere lfd can see, so lfd-backed live
  status (badges, cross-wave rollups) is blind to client-launched sessions. This
  is the concrete dependency on [[architecture]]'s session-registry work
  (`active_sessions_by_worktree`, self-registration on start). Until it lands,
  Concerto's status is `tmux has-session` only.
- **Launch flows beyond `/goal`.** The palette should run any `lf <step-or-flow>`
  in an embedded pane with the right worktree — not just the goal loop. The old
  lfd palette-create path (`POST /v0/terminal-sessions`) was trimmed with the
  native surface; the replacement is a bundled-`lf` exec mirroring the
  `lf goal --tmux` pattern.
- **Reattach across restarts.** Close Concerto, reopen, panes reattach; tmux is
  the source of truth, the embedded view is a client.
- **Multi-agent dispatch** — pick Claude / Codex / OpenCode per launch; the pane
  header shows which provider is running.
- **Workspace layout** — multiple panes per wave (split, tab), layout serialized,
  next launch reopens the same arrangement.

## Known gap: goal resolution in the launched worktree

Demo (2026-07-03) surfaced a real gap: `launch_wave_agent_session` /
`lf goal --tmux` resolves the goal from the *conventional* main-derived sibling
worktree (`loopflow.<wave>`), not from `RepoWork.local_worktree`. When a wave's
goal was authored in a dev checkout surfaced via `CONCERTO_DEV_WAVE_REPO`, the
main-derived sibling is a stale unrelated branch with no `wave/<name>/GOAL.md` →
"goal not found." Only `goals` launched cleanly in the demo because its sibling
happened to carry the goal file.

Open question: should launch trust `RepoWork.local_worktree` instead of
recomputing by naming convention? (That alone wouldn't fix waves whose
`local_worktree` also lacks the goal — the deeper gap is dev-checkout goals not
reaching the launched worktree. This ties into [[2-lfd-owned-wave-identity]]:
once GOAL/MEMORY are lfd-owned and materialized into the launched worktree, the
resolution ambiguity goes away.)

## Done when

- Palette launches arbitrary flows/steps into embedded panes, not just `/goal`.
- Sessions survive Concerto restart and reattach cleanly.
- Multi-agent dispatch is visible in the pane header.
- Layout persists per wave across launches.
- lfd-backed live status reflects `lf`-launched sessions (via the session registry).
- Goal resolution lands in the actually-launched worktree, no "goal not found."
