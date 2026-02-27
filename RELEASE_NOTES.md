# v0.9.3

Concerto ships on iOS, agents learn to remember across sessions, and Chords let you group waves into coordinated workstreams. This release also adds a provider auth broker, hardens token validation, and gates irreversible publishes behind GitHub Release success.

## New capabilities

- **Concerto on iOS.** Multiplatform build with shared state in LoopflowCore. Run on iPhone and iPad simulators with `uv run python scripts/concerto-dev.py run-ios`. macOS unchanged.
- **Portfolio dashboard.** Concerto opens to a live portfolio view — repo cards with wave status, blocked counts, and diff totals. Click a wave to open it; `+` to add repos from `~/src`.
- **Bundled daemon.** Concerto embeds `lfd` — no external install required. Open a repo and a local daemon starts automatically. Optional CLI symlink install for `lf` + `lfd`.
- **Chords.** Group related waves into chords via the API. CRUD endpoints, membership management, and a Python client (`lfq`).
- **Quote-replies.** Select text in an assistant bubble to open a reply composer. Queue text replies and emoji reacts, then send as one structured message.
- **Action buttons.** Agents surface suggested next actions as tappable buttons — macOS and iOS. Tap one to send it as your next message.
- **Wave memory.** Agents read persistent memory from `wave/<wave>/MEMORY.md`. Memory appears in the prompt automatically for wave-scoped steps.

- **Surface-adaptive prompts.** Prompts now adapt to where they run — CLI, headless daemon, Concerto desktop, or Concerto iPhone. Replaces the old `run_mode` string.
- **Composable direction groups.** `lf review -d infra` expands to security + performance + reliability + observability. Stack groups freely: `lf implement -d ux,clarity`.
- **Auth broker.** `lfq auth github`, `lfq auth claude`, `lfq auth codex` — connect providers in your browser. `lfq auth status` shows what's connected.
- **OpenCode harness.** Third adapter validates provider-agnostic sessions. OpenCode communicates via HTTP+SSE, a different transport from Codex and Claude.
- **Remote deployment.** EC2 dogfood lane with smoke test and Caddy TLS config. Deploy with Docker Compose and validate from your laptop.
- **Remote connection seeding.** Drop a `~/.lf/concerto.yaml` with host and port — Concerto boots straight into remote mode. No manual setup on managed machines.

## Improvements

- **`lf ops release`** runs the full release workflow in one command: sync main, worktree, generate notes, commit, tag, and push.
- **Flow naming clarity.** `ship` (headless) is now `build`. `design-ship-review` is now `ship`. `lf flow build` = implement → compress → gate → update-wave.
- **Interactive flow steps** route through session orchestration — Concerto joins inline, and on completion the flow auto-commits and advances.
- **Wave names route into PR titles** instead of commit messages, so commits stay descriptive and PRs stay traceable.
- **Worktrees check out existing remote branches** by name — `lf ops wt create jack-heart.mobile` finds and tracks the remote branch.
- **Orphaned OpenCode servers** are reaped on `lfd` restart. A runtime registry tracks which servers belong to which daemon process.
- **`lfq` is available in the lfd Docker image** alongside `lf` and `lfd`.
- **Codex flags updated** to the new sandbox and approval API (`--dangerously-bypass-approvals-and-sandbox`, `--ask-for-approval never`).
- **PR and commit message generation** now validates LLM output as structured JSON — no more silent garbage from the plaintext fallback parser.

## Security

- **Bearer token pre-validation.** Malformed authorization headers (wrong scheme, whitespace, control chars, overlength) are rejected before reaching any provider.
- **Live auth contract testing.** `test_auth_live_contract.py` validates all three providers end-to-end with CLI transcripts and credential tree snapshots.
- **Irreversible publishes gated on GitHub Release.** Crates.io and PyPI uploads only run after the GitHub Release job succeeds.

## Infrastructure / reliability

- **Session event delivery hardened.** Events flow through unbounded `mpsc` instead of `broadcast` — no dropped events under load. Crash recovery backfills lagged consumers.
- **Contract newtypes.** Prompt pipeline enforces gather → budget → format ordering at the type level (`GatheredContext` → `BudgetedContext` → `RenderedPrompt`).
- **OpenCode schema pinned** to canonical field names with recorded conformance traces. Defensive multi-key fallbacks removed.
- **Legacy Rust agent/chat modules removed.** `agent::anthropic`, `agent::tools`, `chat::contract`, and the `lf-agent` binary are gone — functionality was superseded.
- **Docker executor decomposed.** `docker.rs`, `store/mod.rs`, and `engine/agent.rs` split into focused modules for lower blast radius on future changes.
- **DMG building moved to CI.** Local install simplified to `python3 scripts/install.py local`. Screenshots prefer the installed app over DerivedData builds.