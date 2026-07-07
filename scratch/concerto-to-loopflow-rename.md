# Rename Concerto → Loopflow (+ Mac/iOS app split)

Branch `jack-heart/concerto.loopflow-rename`, stacked on `jack-heart/concerto`.

## Why

Kill the "Concerto" app codename and the never-built "Symphonia" enterprise-library
idea. There is one library — `Loopflow` — and thin per-platform app shells. iOS
comes back as a first-class app.

## Target topology (was: LoopflowCore + single Concerto app)

| Target | Was | Contents |
|---|---|---|
| `Loopflow` (library) | `LoopflowCore` | core models/services/design + 6 pulled-up cross-platform app files + shared bootstrap |
| `LoopflowMac` (app) | `Concerto` (macOS half) | `Platform/macOS/*`, macOS `@main`, macOS-only helpers (`PortfolioService`, tmux cleanup, PATH enrich) |
| `LoopflowiOS` (app) | `Concerto` (iOS half) | `Platform/iOS/*`, iOS `@main` |
| `LoopflowTests` | `ConcertoTests` | unit tests, host = LoopflowMac |
| `LoopflowUITests` | `ConcertoUITests` | macOS UI tests |

Everyone `import Loopflow`. Products both ship as "Loopflow.app". SPM (`swift build`,
unit tests, macOS dev) + xcodegen `project.yml` (Xcode, real iOS build) both updated.

### Pulled up into `Loopflow` library (SPM: shared files live in one target)
`Flags.swift`, `SessionNotifications.swift`, `Views/LiveOutput.swift`,
`Views/PlatformHelpers.swift` (already `#if os(macOS)`-guarded), `Models/PortfolioRepo.swift`,
`State/PortfolioRepoState.swift`, and shared `AppBootstrap` (font reg, AppRuntime,
LaunchArguments, AppearanceMode ext). macOS-only bootstrap bits stay in LoopflowMac.

## What "Concerto" means where

| Referent | Treatment |
|---|---|
| App / library / types / UI strings (`Concerto`) | → `Loopflow` |
| Runtime ids: bundle id, UserDefaults keys, docker volume/socket/db, config paths (`concerto.yaml`), env (`CONCERTO_DEV_WAVE_REPO`), `--concerto-bundled` | → `loopflow` (no back-compat; solo dev, pre-release) |
| `scripts/concerto-dev.py` | → `loopflow-dev.py` |
| Rust `Surface::ConcertoMac/Iphone` + `"concerto_mac"/"iphone"` | → `Surface::Mac/Iphone`, `"mac"/"iphone"` |
| Rust comments naming the app | → `Loopflow` |
| **The wave** `concerto` (`wave/concerto/`, branch, `title_case("concerto")` test, wave/channel-name test fixtures) | **KEEP** — codename for the dev stream, per user |
| Historical `release/*/NOTES.md`, `DECISIONS.md` | **KEEP** |
| Website `/symphonia` page + waitlist | **FLAG, don't touch** — separate product decision |

## Bundle ids
`com.loopflow.concerto` → `com.loopflow.mac`; iOS `com.loopflow.ios`; drop `com.loopflow.core`.

## Coupled non-Swift sites
- `deploy/setup-private-client.sh:174` — `defaults write com.loopflow.concerto concerto.connectionSettings.v2`
- `rust/.../prompt.rs`, `run.rs`, `lf-prompt.rs` — Surface enum
- website (`db.py`, `content.yaml`, `main.py`), python tests, TESTING.md (`Concerto` scheme → `LoopflowMac`)
