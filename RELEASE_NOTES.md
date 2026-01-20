# v0.6.8

Shell integration for automatic directory switching after worktree creation, plus a reworked daemon loop API that uses area as the primary identifier with optional goal composition. Also adds a built-in lint step so `lfops land` works in any repo.

## Changes

- Add `lfops shell install` for auto-cd after `lfops wt create`
- Make area the primary loop identifier in `lfd loop`, with goals as optional `-g` flags
- Support composing multiple goals (e.g., `-g product-engineer -g designer`)
- Inject adaptive mode automatically unless an explicit mode goal is specified
- Add built-in `lint` step so `lfops land` works without a local lint command
- Fast-path lint check in `lfops commit` before invoking agent
