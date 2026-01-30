# v0.7.2

`lf ops next` now handles the full iteration lifecycle—when your PR is already merged, it automatically starts fresh from main instead of failing. No more manual branch cleanup between wave iterations.

## Changes

- `lf ops next` detects merged branches (via PR or squash-merge) and starts fresh from `origin/main` automatically
- Wave metadata updates when starting fresh, so the daemon tracks the correct branch across iterations
- Nested branch timestamps are now parsed correctly, fixing edge cases with multiple wave iterations
