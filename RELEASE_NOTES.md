# v0.9.5

Loopflow 0.9.5 ships an analytics dashboard, hands-free voice input, and conditional flow routing — alongside 40+ improvements to multi-repo support, auth resilience, and developer ergonomics. Waves now auto-watch main and self-heal CI failures out of the box.

## New capabilities

- **Analytics dashboard** in Concerto — toggle between Work and Prompt lenses, pick a period (7d/30d/90d), group by wave/flow/step/model to see where time and tokens go
- **Voice activity detection** — long-press the voice button for hands-free listening; speech is detected, transcribed, and inserted automatically when you pause. Right-click for sensitivity presets (Quiet room / Normal / Noisy)
- **Branch construct for conditional routing** — flows can now fork based on outcomes: `qa → triage → branch(fix or deploy)` routes to different sub-flows depending on what the triage step finds
- **Fast-path step runner** — `lf land` and `lf rebase` run the mechanical operation first, only spinning up an agent if something goes wrong. Steps in flows auto-commit after each step
- **Sandbox executor** — agents can run inside Docker Sandbox microVMs. An adaptive router probes for sandbox support at startup and falls back to standard containers automatically
- **Default wave stimuli** — every new wave ships with Watch (integrates when main advances) and CiFailure (runs ci-fix on red builds). No configuration needed
- **Cross-repo portfolio** — `lf repos add-child` and `lf repos children` create a DAG across repositories. Area targeting like `lf implement -a studio:swift` loads related repo docs automatically
- **Mobile discovery** — lfd publishes presence to studio; iOS auto-discovers running daemons with reachability indicators instead of requiring manual connection setup
- **Quote replies on iOS** — long-press assistant text to select a span, then quote reply or emoji react
- **Inline glance** — click a file in wave diff stats to expand the unified diff in place; click a roadmap item to preview its markdown without leaving the view
- **Live git state during runs** — new commits slide into the log with a brief highlight, diff stats update in place, and a pulsing dot signals the run is active
- **Install onboarding** — `lfd install` walks through connecting providers interactively. Claude and GitHub are required; Codex and OpenCode Zen are optional

## Improvements

- **Directions from config** — `.lf/config.yaml` now drives default directions so every `lf implement` picks them up automatically. `--no-direction` suppresses defaults when you want a clean run
- **Simpler flow commands** — `lf build` instead of `lf flow build`, `lf ship-wave` instead of `lf flow ship-wave`
- **Context audit split** — the token summary now breaks out scratch, wave, and docs separately instead of lumping them together
- **Descendant doc gathering** — `lf agent -a src/api/` now picks up READMEs in subdirectories, not just ancestors
- **Provider catalog** — `curl localhost:4400/v0/providers` returns models, rates, and auth status per provider
- **Cross-platform init** — `lf init` now works on Linux by detecting installed agents instead of trying to install them
- **Worktree pruning** — `lf ops wt list` shows dirty (red) and remote-gone (yellow) states; `lf ops wt prune` groups by reason and fast local listing skips network calls unless you need PR status
- **tmux status format** — `set -g @loopflow_status_format '⚡#{status}'` with variables for branch, step, wave
- **Narrative release notes** — release notes now get full PR bodies and diff stats for richer, thematic summaries
- **Containerized daemon** — Concerto defaults to running lfd inside Docker with `~/src` mounted read-only; Connection Settings offers native fallback if Docker isn't available

## Security

- **Proactive token refresh** — tokens within 20 minutes of expiry are refreshed in the background on a 5-minute interval, preventing mid-run auth failures
- **Git sync hardening** — concurrent runs pushing to the same branch now recover gracefully with dual rebase (sibling + upstream), push escalation on conflict, and agent timeouts
- **Provider-aware refresh failures** — providers that support CLI re-auth (GitHub, Codex) get a re-auth prompt; others degrade to expired status instead of crash-looping

## Fixes

- **Codex compatibility** — replaced removed `--ask-for-approval` flag with `--full-auto` for non-interactive runs
- **Stream parser cleanup** — unknown protocol events are now suppressed inside the parser instead of leaking to callers
- **Release resumption** — `lf ops release publish` detects partially-completed releases and picks up where it left off
