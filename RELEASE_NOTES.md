# v0.9.10

Loopflow 0.9.10 folds the chord model into waves, adds PM integration across Asana, Linear, and Notion, ships an attention queue and wave workspaces in Concerto, and unifies `lf` and `lfd` behind a shared execution engine.

Changes since `v0.9.9`.

## Waves absorb chords

- **Chord-waves replace chord tables** — a chord-wave is now an ordinary wave whose `area` points at `wave/<name>/` directories. Removed ~1400 lines of chord-specific CRUD across Rust, Python, and Swift
- **Gardening replaces tending** — the coordinating flow that lets a chord-wave observe, assess, and mutate member waves. `tend` renamed to `garden` throughout; `scan` + `assess` + `mutate` + `review` are the primitives
- **VSM governance flows** — `govern-identity`, `govern-coordination`, `govern-control`, and `govern-intelligence` each scan, assess, and mutate a different viable-system layer
- **xor / or / loop in flows** — flows can branch on a router step's judgment (`xor`), parallelize (`or`), or repeat until exit (`loop`). `xor` paths can be `silence` — a clean no-op when nothing needs to happen
- **Build / govern / ops categorization** — every step and flow is tagged by agency: `build/` for work you drive, `govern/` for autonomous coordination, `ops/` for side-channel utilities. Concerto's Flows panel groups the catalog the same way

## PM integration

- **Asana, Linear, and Notion providers** — OAuth-only auth, each mapping a wave's `items/` to PM tasks. `lf op pm init`, `pm pull`, `pm export`, `pm push-diff`, `pm status`, all with `--all` variants that sweep every PM-enabled wave
- **`lfq auth asana` / `linear` / `notion`** — connect providers in the browser; `lfq auth status` shows PM providers alongside GitHub/Claude/Codex
- **Branch-locked claims** — Asana working-branch claims prevent two waves from picking the same item concurrently
- **Ingest auto-refreshes PM-backed waves** — pulls from the provider before picking an item, with local-mirror fallback on failure
- **Asana rich text via `html_notes`** — task descriptions preserve markdown formatting on sync, with plaintext fallback for older tasks
- **Priority buckets, Notion page blocks, per-wave bootstrap** — enough provider-specific polish to run real backlogs through the pipe

## Attention queue

- **One inbox for everything needing a human** — code reviews ready to ship, step failures, rebase conflicts, missing PRs, dirty scratch. Items flow `surfaced → viewed → resolved` and auto-resolve when the underlying condition clears
- **Concerto detail pane** — the empty "catch a wave" state is replaced with the attention queue. Items expose contextual actions like **Ship** and **Retry**
- **`/attention` HTTP routes** — `GET /attention`, `GET /attention/history`, `PATCH /attention/{id}` for external clients
- **Algedonic signals with repair backoff** — wave failures escalate through a lineage-aware repair loop with error classification before surfacing to the human

## Wave workspaces and agent embedding

- **Wave workspaces in Concerto** — each wave gets a workspace pane with terminal-native keyboard routing, cached transcript state, and pluggable shells (Ghostty in addition to the built-in terminal)
- **Terminal multiplexer** — multi-pane layout with tmux attach helpers; the lfd side exposes a terminal-attach connection contract
- **Interactive checkpoints** — step agents can pause mid-flow, hand control to a human, and resume where they left off
- **Codex session input over HTTP** — `lfd` accepts Codex session input so agent sessions are controllable via API, not just stdin
- **Connections panel redesign** — cleaner provider auth with Doppler secrets support and on-disk credential detection (Claude/Codex tokens read from `~/.claude/.credentials.json` and `~/.codex/auth.json` before falling back to Keychain)
- **Eager daemon startup** — `BundledDaemonManager` starts at app launch, so provider auth is available before any repo connects

## Shared execution engine

- **One flow engine for `lf` and `lfd`** — flow runs emit runtime journals whether triggered from the CLI or from Concerto, giving both surfaces the same observability
- **`lfd` observes wave CLI runs** — standalone `lf` invocations now report through the same journals the daemon uses for wave runs
- **Prompt system/content split** — assembled prompts use Claude's plan-compatible structure so step agents can be routed through plan mode

## Auth simplification

- **Local vs studio modes** — `lfd` auth collapses to two modes. No more half-wired states
- **PM auth is OAuth-only** — API-key entry removed for Asana, Linear, and Notion; all three flow through `lfq auth` browser handshake

## Wave configuration

- **Wave crons** — supplementary flows scheduled per wave, independent of the worker pool. `workers: 0` + `crons:` is valid for cron-only governance waves
- **Restructured wave items** — items are stored as a directory under `wave/<name>/items/` with structured README frontmatter (Vision, Strategy, Goals, Risks, Metrics)
- **Concurrent ingest coordination** — PM claim prevents two agents from picking the same item simultaneously
- **Garden wave-report step** — reads health signals across all waves for inbox-zero triage

## Review workflow

- **Review splits into `demo` and `code-review`** — `demo` is an experience-first walkthrough of observable changes; `code-review` walks structural and architectural decisions. `review-design` reframed to reshape AI-elaborated design into user intent
- **Review prompts orient before evaluating** — reviews now open with an orientation pass (what changed, what's open, where judgment is needed) instead of narrowing on whatever seemed "most interesting"
- **Structured PR body template** — gate writes `Try it`, `Intent`, `Assumptions`, `Key decisions`, `Not included` sections to `scratch/pr-body.md`; `pr` and `land` consume the cached copy

## Ecosystem

- **gstack workstyle imports** — `lf gstack:office-hours` runs imported steps from external repos. Namespaced flows isolate third-party content from local steps
- **npx skills** — `lf npx:explain-code` fetches a step from the npx ecosystem and runs it
- **Flows catalog in Concerto** — browse the full catalog from the Flows panel; each flow shows parent flows that use it

## Release and CI

- **Nightly regression suite** — automated end-to-end checks feed into a weekly auto-release cadence
- **Decisions ledger shapes release notes** — interactive runs append to `release/unreleased/DECISIONS.md`; `lf op release run` promotes the ledger to `release/v<version>/` and feeds it into the release-notes step. Falls back to merged PR history when the ledger is absent
- **Concerto.app bundles `lf` and `lfd`** — install script no longer required for the embedded CLI and daemon
- **Dependabot hardening** — auto-enable squash merge on open; only close PRs on required-check failure (not flaky optional checks); documented workflow in the repo
- **CI enforces clear scratch before landing** — `scratch/` must be empty before a PR can merge

## Quality of life

- **`lf review-open-work`** — surveys branches, PRs, worktrees, and waves for inbox-zero triage. New `ship` flow pairs it with `land`
- **Rebase-conflict auto-recovery** — `lf op rebase` / `land` / `pr` launch a step agent on conflict instead of failing; the original command retries after resolution
- **Stacked-branch rebase after squash** — fork-point detection via `git cherry` skips commits already absorbed into the target, so B stacked on A rebases cleanly after A squash-merges
- **Fresh-branch preservation** — rotated worktrees are no longer incorrectly pruned; squash-merged status is suppressed while a branch is still tree-equal to main
- **Dirty-file auto-commit** — standalone step runs commit dirty files after completing instead of leaving them loose
- **`lf op wt` prefers exact branch matches** and supports sourced `dev-lf` for in-repo development
- **Rebase notes for already-merged branches** — clearer guidance when a branch's commits are already on main
- **`concerto-dev.py` release script extracted** — `scripts/release-concerto.py` is callable directly from CI, and Concerto font loading works across release, `swift run`, and xcodegen builds
