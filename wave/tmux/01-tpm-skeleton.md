# 01: TPM Skeleton

Plugin entry point, status bar segment, and TPM registration.

## What to build

### Plugin entry point (`loopflow.tmux`)

Shell script at repo root. TPM sources this on install/reload. It:
1. Reads user options (`@loopflow_mode`, `@loopflow_status_format`, `@loopflow_key_prefix`)
2. Sets status bar interpolation (`#{loopflow_status}`)
3. Binds the initial keybindings

```bash
#!/usr/bin/env bash
CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$CURRENT_DIR/scripts/helpers.sh"

# Status bar
tmux set-option -gq @loopflow_status "#{E:@loopflow_status_format}"
tmux set-option -gq status-right "#{loopflow_status} #{E:status-right}"

# Source keybindings
source "$CURRENT_DIR/scripts/keybindings.sh"
```

### Status bar script (`scripts/loopflow-status.sh`)

Called by tmux's status-interval. Returns a short string:

- lf mode: read git branch + check for lf processes → `[lf: main]` or `[lf: feature ▶ implement]`
- Container mode: `lfq --json 2>/dev/null` → `[lf: 3 waves | engbot ▶ 2/4]`
- Fallback: `[lf]` if nothing is running, `[lf: --]` if binary not found

Must complete in <100ms. Cache to `/tmp/loopflow-status-$USER` if needed.

### User options

| Option | Default | What it does |
|--------|---------|-------------|
| `@loopflow_mode` | `auto` | `lf` (no daemon), `container`, or `auto` (detect) |
| `@loopflow_status_format` | `[lf: #{loopflow_waves}]` | Customizable status format |
| `@loopflow_key_prefix` | `l` | Key after tmux prefix for loopflow actions |

### Install instructions (README update)

```bash
# Add to .tmux.conf
set -g @plugin 'loopflowstudio/loopflow.tmux'
run '~/.tmux/plugins/tpm/tpm'

# Or clone manually
git clone https://github.com/loopflowstudio/loopflow.tmux ~/.tmux/plugins/loopflow.tmux
```

## Constraints

- Pure shell scripting. No compiled dependencies for the plugin itself.
- Graceful degradation when `lf`/`lfq` aren't installed — status shows placeholder, keybindings are no-ops with a message.

## Validation

```bash
# Simulate TPM load
tmux source-file loopflow.tmux
tmux display-message -p '#{loopflow_status}'
```

## Done when

- TPM install works (`prefix + I` with the plugin line in `.tmux.conf`)
- Status bar shows wave info
- Plugin loads without errors when `lf` is not in PATH
