# 03: Keybindings

Wave action bindings with explicit mode routing, predictable feedback, and picker fallbacks.

## What to build

### Binding table

All bindings are `prefix + @loopflow_key_prefix + <key>` (default: `prefix + l + <key>`).

| Key | Action | lf mode | Container mode |
|-----|--------|---------|----------------|
| `r` | Run wave/step | `lf <step>` in current pane | `lfq run <wave>` |
| `s` | Stop | kill pane process | `lfq stop <wave>` |
| `o` | Open logs | `lf flow build` output | `lfq logs <wave>` |
| `p` | Open PR | `gh pr view --web` | `lfq show <wave>` → PR URL → open |
| `n` | Next iteration | `lf ops next` | `lfq ... next` |
| `d` | Land | `lf ops land` | `lfq land <wave>` |
| `w` | Pick wave | fzf over git worktrees | fzf over `lfq list --json` |
| `L` | Layout picker | fzf over layout names | same |
| `?` | Show keybinding help | display-message with binding table | same |

### Command routing rules

- Resolve mode once per keypress (`loopflow_mode`).
- Route through a single helper (`loopflow_dispatch <action>`) to avoid drift.
- Never inline large command logic directly in `bind-key` definitions.

### Feedback rules

Every action must end with one of:

1. command launched
2. nothing selected
3. dependency missing
4. mode not available

Use concise `tmux display-message` output for all non-launch outcomes.

### Wave picker (`prefix + l + w`)

Uses `fzf` (or `tmux choose-tree` as fallback) to select a wave/worktree:
- lf mode: lists git worktrees via `lf ops wt list`
- Container mode: lists waves via `lfq list --json`

Selection opens the chosen wave's worktree in a new lf-dev layout.

### Layout picker (`prefix + l + L`)

- Preferred: `fzf` with entries `dev`, `swarm`.
- Fallback: `tmux display-menu`.
- Selection maps to corresponding layout script.

### Mode detection

`scripts/helpers.sh` provides `loopflow_mode()`:
1. If `@loopflow_mode` is set to `lf` or `container`, use that.
2. If `auto`: check if `lfq status` succeeds (container running). If yes, container mode. Otherwise lf mode.

Add timeout budget for detection (250ms default) to avoid keypress lag.

### Binding script (`scripts/keybindings.sh`)

Sourced by `loopflow.tmux`. Reads `@loopflow_key_prefix` and registers all bindings.

Implementation constraints:

- unbind/rebind pattern for idempotent resourcing
- no duplicate bindings after re-source
- support both lowercase and uppercase prefix if configured

## Constraints

- Bindings must not conflict with common tmux defaults or popular plugins.
- `fzf` is a soft dependency — fallback to `tmux choose-tree` or `tmux display-menu`.
- Every binding handles the "nothing selected / no wave running" case with a display-message, not a silent failure.
- Binding actions must not assume container mode unless verified.

## Validation

```bash
tmux source-file loopflow.tmux
# Press prefix + l + ? to see binding help
# Press prefix + l + w to pick a wave
```

Manual checks:

1. Source plugin twice; bindings stay single.
2. Trigger each binding in lf mode.
3. Trigger each binding in container mode.
4. Remove `fzf`; picker fallback still works.
5. Kill daemon in container mode; actions fail-soft.

## Done when

- All bindings in the table work in both modes
- Wave picker shows available waves/worktrees
- `?` shows a readable help overlay
- Bindings are no-ops with a message when prerequisites are missing
- Action dispatch is centralized and easy to extend
