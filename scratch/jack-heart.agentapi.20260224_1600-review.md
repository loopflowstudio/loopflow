# Gate review: wavemodel agent harness + design-first Concerto

## What was implemented

Session-based agent runtime in `lfd` and design-first onboarding in Concerto.

**Rust (runtime):**
- `SessionManager` with full lifecycle: create → start → input → events → stop, plus orphan recovery on daemon restart.
- `prepare_step_prompt()` extracted to `lfd/prompt.rs` — shared prompt assembly for sessions and wave executor, removing duplication between the two paths.
- `LaunchConfig` / `ProcessConfig` / `AgentCapabilities` split replaces the prior tangled config types. Launch carries prompt/model/cwd; process carries execution behavior; capabilities carries provider feature flags.
- Claude and Codex harness implementations with SSE event bridge, provider session ID tracking (for Claude resume), and crash escalation.
- Store additions: session CRUD, event append/list, active-session-for-wave-run lookup, orphan listing by status.
- Stricter validation: `repo_root` must exist and contain `.lf/`; `cwd` must resolve inside `repo_root`.
- `lf release` converted from hardcoded CLI subcommand to agent step (`ops/release.md`), consistent with all other steps.

**Swift (Concerto):**
- `ChatState` manages session lifecycle, SSE streaming with replay/live phases, transcript items, and bounded output.
- `StartWaveView` launches inline design chat directly — no terminal detour.
- `WaveDetailPanel` surfaces wave content (Vision/Goals/Risks/Roadmap) parsed from `wave/<name>/README.md`.
- `WaveContentParser` extracts structured sections from markdown. `WaveSidebar` slimmed down.
- `WaveSchema` model deleted; replaced by `WaveContent` for the design-first path.

## Key choices

| Decision | Why | Alternatives rejected |
|----------|-----|----------------------|
| Server-side prompt assembly | Callers send metadata (step, repo, directions); lfd assembles full prompts. Keeps prompt logic in one place. | Client-assembled prompts (used previously) led to duplicated shaping logic in UI and executor. |
| `LaunchConfig` + `ProcessConfig` split | Prompt content is orthogonal to execution mode. Sessions use LaunchConfig without ProcessConfig; CLI uses both. | Single config struct — too many optional fields, unclear which apply where. |
| Broadcast channel for harness events | Decouples event production (harness) from consumption (persistence + SSE). Handles backpressure with lag warnings. | mpsc — can't have multiple consumers. Direct persistence from harness — couples harness to store. |
| Orphan recovery on startup | Sessions left in Starting/Active after daemon restart are unrecoverable. Mark them Failed with an error event so clients see a clean state. | Leave them hanging — clients poll forever. |
| Design-first onboarding | Users describe intent in natural language; the design step runs inline. Lower barrier than schema-first wave setup. | Schema picker (removed from main path, still available via API). |
| Release as agent step | `lf release` was the only hardcoded CLI command for what should be an agent step. Moving to `ops/release.md` makes it consistent. | Keep the native Rust implementation — adds special-case code for one step. |

## How it fits together

```
Client (Concerto/CLI)
  → POST /v0/sessions {step, repo_root, directions, ...}
    → SessionManager.create_session()
      → validate_repo_root() + resolve_cwd()
      → prepare_step_prompt() → LaunchConfig
      → create harness (Claude/Codex)
      → spawn event bridge (harness → store + broadcast)
      → spawn startup (harness.start(launch))
  → POST /v0/sessions/:id/input {content}
    → SessionManager.send_input() → harness.send_input()
  → GET /v0/sessions/:id/events (SSE)
    → replay from store + live tail from broadcast
```

Sessions are the orchestration boundary for interactive runs. The wave executor uses `prepare_step_prompt()` for auto/headless runs through `build_step_prompt()` in `executor/helpers.rs`.

## Risks and bottlenecks

- **Runtime drift**: Sessions and wave executor share `prepare_step_prompt()` but diverge after that (sessions use harness, executor uses `launch_agent()`). Convergence is planned but not yet landed.
- **Single active session per wave run**: Enforced by DB check, not lock. Race window exists between check and insert. Acceptable for current usage patterns.
- **Markdown parsing on main actor**: `WaveContentParser` reads files synchronously. Fine for typical wave README sizes; could hitch on very large files. Noted for future async migration.
- **String-based status comparison in Swift**: `ChatState` compares session/turn status as strings (`"active"`, `"completed"`). Works because the Rust side serializes via `serde(rename_all = "snake_case")`. A typed enum on the Swift side would be more robust but requires more mapping infrastructure.

## What's not included

- Wave executor routing through session orchestration (planned convergence work).
- `workspace_changed` signaling for UI refresh on file updates.
- Per-tab/per-wave step selection in Concerto (currently defaults to `design`).
- Gemini/OpenCode harness implementations (providers return `ProviderNotImplemented`).
- Wave content refresh/watch behavior.

## Validation

All CI-equivalent checks pass locally:

| Check | Result |
|-------|--------|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `cargo test --all` | 448 passed (2 docker tests skipped — no socket on dev machine) |
| `uv run pytest python/tests/` | 47 passed |
| `swift test --package-path swift` | 131 passed |
| `tests/e2e/test_smoke.sh` | pass |
