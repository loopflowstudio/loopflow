# 05: Polish

Follow-ups from the 01–03 review that improve the shipped plugin surface.

## What to build

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

- At least one interactive flow is tested automatically
