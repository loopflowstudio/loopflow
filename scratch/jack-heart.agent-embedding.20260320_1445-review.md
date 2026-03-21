# Branch review — multiplexed wave workspace and terminal-session attention plumbing

## What was implemented

This branch turns Concerto's repo window into a real wave workspace instead of a single detail panel. Selected waves now open into a persisted multiplexer layout with default **Roadmap**, **Runs**, and **Terminal** panes; queue-empty repo windows show wave overview cards; and interactive waves still auto-take over with the live session view when human input is needed.

On the daemon side, `lfd` now has first-class terminal-session models, storage, events, and HTTP routes. The executor creates interactive attention items when a flow waits for human input, enriches them with step-scoped context, and resolves them when the terminal session completes.

## Key choices

- **Executor owns interactive attention lifecycle.** `WaitInteractive` now creates the attention item and terminal session together, and terminal completion resolves the item immediately instead of waiting for reconciliation.
- **Coarse attention kinds, typed context.** The branch keeps `interactive` / `algedonic` as the stable top-level kinds and pushes step-specific detail into `context.step`, `design_path`, and `mutation_summary`.
- **Per-wave workspace persistence.** Multiplexer layouts and focused panes persist in `UserDefaults` by repo + wave, while terminal session ordering/selection persists separately so workspace state survives reloads without coupling pane layout to session lifecycle.
- **Interactive takeover beats workspace chrome.** When a wave is waiting on a human, `RepoState` routes directly into the interactive session instead of leaving the user inside the generic multiplexer.

## How it fits together

Rust adds the backend contract: terminal session rows in the store, `/v0/terminal-sessions/*` routes, wrapped attach/complete commands, and executor hooks that create/resolve interactive attention items around waiting steps. Swift consumes that contract through `LoopflowCore` models/services/stores, then renders it in two places: the attention queue detail pane and the per-wave multiplexer workspace.

## Risks and bottlenecks

- **macOS UI runner instability:** `xcodebuild test` still builds and runs the unit/app suites here, but `ConcertoUITests-Runner` exits early before finishing bootstrap on this machine.
- **Session completion wrapper is shell-mediated:** terminal completion depends on the wrapped `zsh` command reaching the local HTTP completion endpoint with the generated token.
- **Preview detail depends on artifacts existing:** `review-design` previews are only as good as the design doc on disk, and `wave/review` summaries depend on the mutation summary artifact being present.
- **Persisted pane layouts may need migration later:** pane type/config changes will need explicit migration if the multiplexer schema evolves.

## What's not included

- Daemon-managed tmux sessions embedded directly into the default dashboard for every wave
- Calibration-specific queue/detail UI beyond the new generic interactive attention plumbing
- A fix for the local `ConcertoUITests-Runner` bootstrap crash observed under `xcodebuild test`
