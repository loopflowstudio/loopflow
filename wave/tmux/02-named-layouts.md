# 02: Named Layouts

Pre-built pane arrangements with deterministic behavior and mode-aware command bootstrapping.

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

### Shared layout runtime contract

Every layout script must:

1. Create a **new tmux window** with a stable name.
2. Use current repo path (or explicit argument) as working directory.
3. Detect mode once via `loopflow_mode`.
4. Seed panes with commands but not force execution when unsafe.
5. Print one-line failure reason via `tmux display-message` when blocked.

Suggested window names:

- `lf-dev`
- `lf-swarm`
- `lf-flow`

### Layout dispatch

Each layout is a standalone script (`scripts/layouts/lf-dev.sh`, etc.) that creates the pane arrangement using `tmux split-window`, `tmux send-keys`, etc.

Callable via:
- Keybinding: `prefix + L + d` (dev), `prefix + L + s` (swarm), `prefix + L + f` (flow)
- CLI: `lf tmux dev`, `lf tmux swarm`, `lf tmux flow`

### Smart defaults

- Layouts detect the current working directory and use it as the repo root
- In container mode, layouts use `lfq` commands. In lf mode, direct `lf` commands.
- Small terminals (<120 cols) get simplified layouts (fewer panes)
- If editor command is missing, leave pane as shell and print hint.

### Geometry policy

- Preferred baseline: 200x50 terminal.
- Minimum supported: 120x30.
- Below minimum: create simplified 2-pane fallback instead of failing.

### Command templates

Keep command strings centralized in one helper file (avoid drift):

- `lf-dev`: `lf <step>` / `lfq logs <wave>`
- `lf-swarm`: `lf implement -a ...` variations
- `lf-flow`: `lf flow build` + `lfq list --json`

## Constraints

- Layouts create a new tmux window, don't disrupt the current one.
- Each layout script is self-contained — no shared state between layouts.
- Panes run real commands, not wrappers. Users can interact with them normally.
- Re-running the same layout should open a fresh window, not mutate existing windows.

## Validation

```bash
scripts/layouts/lf-dev.sh
scripts/layouts/lf-swarm.sh
scripts/layouts/lf-flow.sh
```

Manual checks:

1. Run each layout in lf mode and container mode.
2. Run on <120-column terminal and verify simplified fallback.
3. Run with missing `lfq`; layout still opens with useful pane shells.

## Done when

- All three layouts create correct pane arrangements
- Layouts degrade gracefully on small terminals
- Keybindings and CLI dispatch both work
- Window names are stable and discoverable
