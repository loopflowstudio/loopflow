# concerto-next

Multi-platform wave experience for Concerto.

## Contents

| Doc | Description |
|-----|-------------|
| [00-overview](00-overview.md) | Conduct & Improvise modes, core concept |
| [01-platform](01-platform.md) | Platform story, server evolution, client strategy |
| [02-auth](02-auth.md) | Auth model (local = none, remote = Loopflow) |
| [03-conduct-ux](03-conduct-ux.md) | Dashboard, connect, continue, land |
| [04-improvise-ux](04-improvise-ux.md) | Area picker, step runner, transitions |
| [05-remote-terminal](05-remote-terminal.md) | Terminal streaming architecture |
| [06-data-structures](06-data-structures.md) | Wave types, key functions, constraints |
| [07-ux-experiments](07-ux-experiments.md) | Personas, repeatable test scripts |
| [08-notifications](08-notifications.md) | Push notification architecture |
| [09-phasing](09-phasing.md) | Phases 1-4, done criteria |

## Summary

Two modes, same UI:
- **Conduct**: Dashboard-first, connect when needed, land PRs
- **Improvise**: Create wave, run steps manually, discover

Four phases:
1. macOS local (Conduct + Improvise)
2. Remote access foundation (auth, terminal streaming)
3. Mobile (iOS/iPad)
4. Rust lfd
