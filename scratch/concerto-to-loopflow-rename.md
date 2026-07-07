# Rename Concerto → Loopflow (+ Mac/iOS app split)

Branch `jack-heart/concerto.loopflow-rename`, stacked on `jack-heart/concerto`.

## Rebase note (2026-07-07)

Parent branch `jack-heart/concerto` landed on main as a squash (`e6bca5e5`,
"concerto: show objective and project plan"). Its tree is byte-identical to this
branch's pre-rename tip `9c322bf5` except for scratch deletions, so replaying the
wave-viewer commits would only re-conflict with already-landed work. Rebased with
`git rebase --onto origin/main 9c322bf5` to drop those duplicates and replay only
the 7 rename commits — clean, no conflicts. The rename shortened the `Surface`
test array below rustfmt's width; fixed with `cargo fmt` in a follow-up commit.
Full gate green (cargo build/fmt/clippy, Python 54, Swift build + 304 tests,
website).

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

### Library additions (kept minimal after a correction)
Only `AppBootstrap.swift` is pulled into the library — the genuinely shared
app-startup helpers (`bootstrapLoopflowApp`, `AppRuntime`, `LaunchArguments`,
`AppearanceMode.resolvedTheme`, font registration via a `SWIFT_PACKAGE`-aware
`Bundle` accessor). Fonts moved to the library as a resource.

First attempt over-pulled `Flags`, `SessionNotifications`, `PlatformHelpers`,
`LiveOutput`, `PortfolioRepo`, `PortfolioRepoState` into the library. That broke
the build: `PortfolioRepoState` calls `LocalWaveAgentLauncher` (a per-platform
`enum` in each app target — the library can't see app code), and the rest forced
needless cross-module `public`. Fix: relocate each to its actual consumer —
`Flags`/`SessionNotifications`/`PlatformHelpers`/`PortfolioRepo`/`PortfolioRepoState`
→ `LoopflowMac` (mac-only), `LiveOutput` → `LoopflowiOS` (iOS-only). The library
stays the clean cross-platform core.

Lesson: "outside `Platform/` in the old single target" ≠ cross-platform. And
`swift build ... | tail` masks the real exit code — verify builds without a pipe.

### Xcode module-name collision (the subtle one)
Setting `PRODUCT_NAME: Loopflow` on the app made its **Swift module** `Loopflow`
too (Xcode derives the module name from PRODUCT_NAME by default), colliding with
the framework module → `import Loopflow` in app files became a self-import
("file is part of module 'Loopflow'; ignoring import") and every framework
symbol read as "cannot find in scope." SPM was unaffected (module = target name).
Fix: `PRODUCT_MODULE_NAME: LoopflowMac` / `LoopflowiOS` so the product ships as
`Loopflow.app` but the module stays distinct from the framework.

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
