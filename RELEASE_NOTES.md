# v0.9.2

Chords let you compose waves together — join independent waves into ensembles and add cross-wave stimuli so one wave can listen to another's progress. Directions are now composable quality groups (`-d ux`, `-d craft`) instead of fixed role personas, and wave-scoped agents gain persistent memory and injectable `/slash` skills.

## New capabilities

- **Chords** — join waves into ensembles with `loopflow.join("designer", "infra")` and add cross-wave awareness with `loopflow.add_stimulus("designer", kind="listen", source_wave_id="infra")`
- **Composable direction groups** — stack quality lenses like `-d ux,clarity` or `-d infra`; groups expand recursively into member directions
- **Skill injection** — built-in steps and directions appear as `/slash` commands in Claude Code when `inject_skills: true` is set in `.lf/config.yaml`; always on for `lfd` sessions and wave-spawned agents
- **Wave memory** — agents read persistent context from `wave/<wave>/MEMORY.md`, carried across runs
- **Interactive flow steps** — when a flow hits an interactive step, lfd creates a session and Concerto joins it inline; on completion the flow auto-commits and advances
- **Portfolio dashboard** — Concerto opens a live dashboard of your repos instead of a welcome screen; click a wave row to jump in, click `+` to add repos from `~/src`
- **Bundled daemon** — Concerto ships `lfd` inside the app bundle; no separate install needed, optional CLI symlink to `~/.local/bin`
- **iOS support** — Concerto builds and runs on iOS Simulator (`uv run python scripts/concerto-dev.py run-ios`)
- **Remote editor launch** — open Terminal, Cursor, VSCode, or Zed connected to a remote worktree directly from Concerto
- **OpenCode harness** — third session adapter validates the API is provider-agnostic; OpenCode sessions communicate via HTTP+SSE
- **`lf release`** — generate release notes from merged PRs and auto-tag on merge (`lf release`, `lf release minor`, `lf release 0.9.2`)
- **`lf ops release`** — single command for the full release workflow: sync main, worktree, generate notes, commit, create and land PR

## Improvements

- **Flow renames** — `ship` (headless) is now `build`; `design-ship-review` (interactive) is now `ship`; names match intent
- **Filesystem wave config** — wave configuration lives in `wave/<name>/<name>.yaml` on disk; no schema resolution layer
- **Docker fork parity** — fork flows now work in the Docker executor, matching native mode behavior
- **Unified fork executor** — CLI, daemon, and Docker all follow one shared contract for worktree paths, branch identity, and workspace lifecycle
- **Worktrees from remote branches** — `lf ops wt create jack-heart.mobile.20260225` checks out an existing remote branch instead of creating a new one
- **Hardened message generation** — PR and commit messages now require structured JSON from the agent; the unsafe plaintext fallback parser is gone
- **`provider` → `harness`** — session creation uses `"harness": "claude"` instead of `"provider": "claude"`; shared prompt assembly across harnesses
- **DMG builds moved to CI** — local install simplified to `python3 scripts/install.py local`

## Security

- **API surface gating** — configurable limits on JSON body size, WebSocket frame/message size, and queue depth via `lfd.yaml` or environment variables
- **Credential hygiene** — secrets carried in `SecretString` (zeroized on drop, redacted in logs); query-param credentials rejected; `lfd token rotate` for static token rotation
- **Bearer token pre-validation** — malformed authorization headers (wrong scheme, embedded whitespace, overlength) rejected before reaching auth providers

## Infrastructure / reliability

- **Session hardening** — event delivery uses unbounded `mpsc` instead of `broadcast` (no dropped events); crash recovery backfills lagged subscribers; conformance test suite added
- **Orphan reaping** — lfd startup detects and terminates orphaned `opencode serve` processes from previous instances
- **Contract hardening** — prompt pipeline enforces gather → budget → format ordering at the type level via newtypes; Docker metadata conventions enforced by invariant tests
- **E2E test harness** — hermetic smoke tests build lfd from source against an isolated environment and exercise the full wave CRUD lifecycle
- **Legacy cleanup** — removed standalone Rust agent/chat modules and `portable-pty` dependency; functionality superseded by the session API