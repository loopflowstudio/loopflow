# `@loopflow_status_format` customization

Picked from `wave/tmux/05-polish.md`.

## Problem

The tmux option `@loopflow_status_format` is documented in the phase 01 spec but not wired up. Status format is hardcoded in `loopflow-status.sh`. Users who set the option get no effect.

## What to build

Either implement the format template so `@loopflow_status_format` is respected by the status script, or remove the option from docs.

Current behavior: `[lf: <branch>]` or `[lf: N waves | name]` hardcoded.
Desired: user sets `@loopflow_status_format` and the script respects it.

## Constraints

- No new dependencies.
- Don't break existing structural tests.

## Done when

- Status format is customizable via `@loopflow_status_format`, or the option is removed from docs
