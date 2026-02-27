# `@loopflow_status_format` customization

## Context

The tmux option `@loopflow_status_format` is documented in the phase 01 spec but not wired up. Status format is hardcoded in `loopflow-status.sh`. Either implement the format template or remove the option from docs.

Current behavior: `[lf: <branch>]` or `[lf: N waves | name]` hardcoded.
Desired: user sets `@loopflow_status_format` and the script respects it.

## Constraints

- No new dependencies.
- Don't break existing structural tests.

## Done when

- Status format is customizable via `@loopflow_status_format`, or the option is removed from docs
