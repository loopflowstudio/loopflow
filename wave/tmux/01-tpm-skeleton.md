# 01: TPM Skeleton

Plugin entry point, status plumbing, mode detection helpers, and install ergonomics.

## What to build

### Plugin entry point (`loopflow.tmux`)

Shell script at repo root. TPM sources this on install/reload. It:
1. Defines defaults for tmux options if unset.
2. Sources `scripts/helpers.sh` and `scripts/keybindings.sh`.
3. Registers status interpolation variables.
4. Is safe to source multiple times (idempotent).

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

### Helper contract (`scripts/helpers.sh`)

Provide these functions. Keep them small and side-effect free:

- `loopflow_mode`: returns `lf` or `container`
- `loopflow_has_cmd <name>`: fast command-exists check
- `loopflow_status_cache_read`: read cache + TTL validation
- `loopflow_status_cache_write`: atomic cache write
- `loopflow_display`: wrapper around `tmux display-message`

### Status interpolation contract

Expose both:

- `#{loopflow_status}`: rendered display string
- `#{@loopflow_status_format}`: customizable format template

If custom format is invalid, fallback to default.

### Status bar script (`scripts/loopflow-status.sh`)

Called by tmux's status-interval. Returns a short string:

- lf mode: read git branch + check for lf processes → `[lf: main]` or `[lf: feature ▶ implement]`
- Container mode: `lfq --json 2>/dev/null` → `[lf: 3 waves | engbot ▶ 2/4]`
- Fallback: `[lf]` if nothing is running, `[lf: --]` if binary not found

Must complete in <100ms hot path. Cache format should include:

- `generated_at` (epoch)
- `mode` (`lf`|`container`)
- `status_text`
- `source` (`live`|`cache`|`fallback`)

Cache location: `/tmp/loopflow-status-$USER.json` (single file).
TTL: 2s default.

On cache use, append subtle stale marker only when older than TTL (`~` or similar).

### User options

| Option | Default | What it does |
|--------|---------|-------------|
| `@loopflow_mode` | `auto` | `lf` (no daemon), `container`, or `auto` (detect) |
| `@loopflow_status_format` | `[lf: #{loopflow_waves}]` | Customizable status format |
| `@loopflow_key_prefix` | `l` | Key after tmux prefix for loopflow actions |
| `@loopflow_status_ttl_ms` | `2000` | Cache TTL to limit subprocess churn |
| `@loopflow_status_timeout_ms` | `250` | Per-command timeout budget |

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
- Plugin load must not mutate user tmux options outside loopflow-owned keys.
- Do not auto-start containers during plugin load.

## Failure handling

- Missing `tmux` binary in script execution path: exit 0 with no output.
- Missing `lf` and `lfq`: return fallback status token.
- Slow `lfq` call: timeout + use cache.
- Invalid JSON from `lfq --json`: ignore and fallback.

## Validation

```bash
# Simulate TPM load
tmux source-file loopflow.tmux
tmux display-message -p '#{loopflow_status}'
```

Manual checks:

1. Source file twice; verify no duplicate keybinding side effects.
2. With `lfq` unavailable, status still renders in <100ms.
3. With stale cache and dead daemon, status uses fallback not hang.

## Done when

- TPM install works (`prefix + I` with the plugin line in `.tmux.conf`)
- Status bar shows wave info
- Plugin loads without errors when `lf` is not in PATH
- Helper functions exist and are used by later phases
