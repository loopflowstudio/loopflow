# 02: Named Layouts

Pre-built tmux pane arrangements for common loopflow workflows.

## What to build

### Three layouts in `scripts/layouts/`

**lf-dev** — single wave focus:
```
+------------------+----------------------+
|                  |                      |
|   editor         |   lf <step>          |
|   (user's $EDITOR|   (agent output)     |
|    or empty)     |                      |
|                  +----------------------+
|                  |   shell              |
|                  |   (run/test/git)     |
+------------------+----------------------+
```

**lf-swarm** — parallel agents (inspired by DHH's `tsl`):
```
+----------------------------------------+
|   leader pane (lf flow build)          |
+------------+------------+--------------+
| worker 1   | worker 2   | worker 3     |
| lf impl    | lf impl    | lf impl     |
| -a src/    | -a tests/  | -a docs/    |
+------------+------------+--------------+
```

**lf-flow** — watch a flow execute:
```
+------------------+----------------------+
|  lfq list        |   flow output        |
|  (refreshing)    |   (lf flow build)    |
|                  |                      |
|                  +----------------------+
|                  |   lazygit / shell    |
+------------------+----------------------+
```

### Layout dispatch

Each layout is a standalone script (`scripts/layouts/lf-dev.sh`, etc.) that creates the pane arrangement using `tmux split-window`, `tmux send-keys`, etc.

Callable via:
- Keybinding: `prefix + L + d` (dev), `prefix + L + s` (swarm), `prefix + L + f` (flow)
- CLI: `lf tmux dev`, `lf tmux swarm`, `lf tmux flow`

### Smart defaults

- Layouts detect the current working directory and use it as the repo root
- In container mode, layouts use `lfq` commands. In lf mode, direct `lf` commands.
- Small terminals (<120 cols) get simplified layouts (fewer panes)

## Constraints

- Layouts create a new tmux window, don't disrupt the current one.
- Each layout script is self-contained — no shared state between layouts.
- Panes run real commands, not wrappers. Users can interact with them normally.

## Validation

```bash
scripts/layouts/lf-dev.sh
scripts/layouts/lf-swarm.sh
scripts/layouts/lf-flow.sh
```

## Done when

- All three layouts create correct pane arrangements
- Layouts degrade gracefully on small terminals
- Keybindings and CLI dispatch both work
