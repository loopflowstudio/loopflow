# 02: Deployment Collapse

**Finish line:** Three blessed deployment configs replace the combinatorial matrix. Users pick solo, team, or CI — not auth × storage × isolation × agent independently.

## Context

Currently users can modulate each deployment dimension independently, producing configurations nobody has tested. The blessed configs collapse this to three tested paths.

## What to build

1. **Define three configs:**
   - `solo`: local lfd, local agents, file-based state, local token auth
   - `team`: shared lfd, WorkOS auth, postgres, container isolation
   - `ci`: headless lfd, single-run mode, no persistence, static token

2. **Configuration entrypoint.** `lfd --mode solo|team|ci` or `LFD_MODE` env var. Each mode sets all downstream config. Individual dimension overrides still possible but undocumented — escape hatch, not primary path.

3. **Delete combinatorial config code.** Remove the abstraction layers that let each dimension vary independently. Replace with mode-driven defaults.

4. **Update Docker Compose and deployment docs** to use the three modes.

## Done when

- `lfd --mode solo` starts with all defaults
- `lfd --mode team` starts with postgres, auth, isolation
- `lfd --mode ci` starts headless with static token
- No per-dimension configuration in docs or examples
