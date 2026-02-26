# 05: Polish

Follow-ups from the 01–03 review that improve the shipped plugin surface.

## What to build

### `@loopflow_status_format` customization

The tmux option is documented in phase 01 spec but not wired up. Status format is hardcoded in `loopflow-status.sh`. Either implement the format template or remove the option from docs.

Current behavior: `[lf: <branch>]` or `[lf: N waves | name]` hardcoded.
Desired: user sets `@loopflow_status_format` and the script respects it.

### Interactive test coverage

`tmux-review.py` verifies structure (bindings exist, scripts load) but doesn't exercise interactive flows: pickers, layout creation, mode switching. Add automated interactive tests when tmux is available in CI.

Priorities:
- picker fallback path (fzf missing)
- layout creation and pane arrangement verification
- mode switching between lf and container

## Constraints

- No new dependencies.
- Don't break existing structural tests.

## Done when

- Status format is customizable via `@loopflow_status_format`, or the option is removed from docs
- At least one interactive flow is tested automatically
