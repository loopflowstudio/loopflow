# 03: Keybindings

Wave action bindings that dispatch to `lf` or `lfq` depending on mode.

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

### Wave picker (`prefix + l + w`)

Uses `fzf` (or `tmux choose-tree` as fallback) to select a wave/worktree:
- lf mode: lists git worktrees via `lf ops wt list`
- Container mode: lists waves via `lfq list --json`

Selection opens the chosen wave's worktree in a new lf-dev layout.

### Mode detection

`scripts/helpers.sh` provides `loopflow_mode()`:
1. If `@loopflow_mode` is set to `lf` or `container`, use that.
2. If `auto`: check if `lfq status` succeeds (container running). If yes, container mode. Otherwise lf mode.

### Binding script (`scripts/keybindings.sh`)

Sourced by `loopflow.tmux`. Reads `@loopflow_key_prefix` and registers all bindings.

## Constraints

- Bindings must not conflict with common tmux defaults or popular plugins.
- `fzf` is a soft dependency — fallback to `tmux choose-tree` or `tmux display-menu`.
- Every binding handles the "nothing selected / no wave running" case with a display-message, not a silent failure.

## Validation

```bash
tmux source-file loopflow.tmux
# Press prefix + l + ? to see binding help
# Press prefix + l + w to pick a wave
```

## Done when

- All bindings in the table work in both modes
- Wave picker shows available waves/worktrees
- `?` shows a readable help overlay
- Bindings are no-ops with a message when prerequisites are missing
