# Review: `@loopflow_status_format` customization

## What was implemented

Status format is now customizable via `@loopflow_status_format`. The tmux option accepts a template string with `#{variable}` placeholders that get substituted at render time.

Variables: `#{status}` (computed text — branch, step, wave info), `#{branch}`, `#{step}`, `#{waves}`, `#{wave}`.

Default: `[lf: #{status}]` — produces identical output to the previous hardcoded behavior.

## Key choices

**Single format string, not per-mode templates.** One `@loopflow_status_format` option covers both lf and container mode. The `#{status}` variable provides the smart default; granular variables (`#{branch}`, `#{step}`, `#{waves}`, `#{wave}`) give full control. Adding separate `@loopflow_status_format_lf` and `@loopflow_status_format_container` options would be more flexible but the complexity isn't justified — users who need different formats per mode can use tmux conditionals.

**No conditional syntax.** Considered `#{?step, ▶ #{step}}` for conditional inclusion but it adds parser complexity to a shell script that must complete in <100ms. Users who want conditional formatting can use the `#{status}` variable which already handles step presence/absence.

**Shell parameter expansion for substitution.** Uses bash `${fmt//pattern/replacement}` — fast, no external dependencies, no subshells. All five substitutions happen in-process.

## How it fits together

`loopflow.tmux` sets the default option. `loopflow-status.sh` adds `loopflow_apply_format()` which reads the option and performs substitution. Both `generate_lf_status()` and `generate_container_status()` call it instead of hardcoding `echo "[lf: ...]"`. The cache layer is unchanged — it stores the final formatted string.

## Risks and bottlenecks

- **tmux option read on every render.** `loopflow_get_option` calls `tmux show-option` which is a subprocess. This already happens for mode, TTL, and timeout — one more call is marginal. The cache layer means this only runs every 2s.
- **No input validation.** Malformed format strings produce garbled output but can't break anything — they're just string substitution.

## What's not included

- Conditional formatting (`#{?var,...}`) — intentionally deferred.
- Per-mode format strings — single option covers both modes.
- Format string validation or error messages for unknown variables.
