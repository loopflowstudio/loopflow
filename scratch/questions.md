# Open questions / assumptions — waveagent-launcher

## `lf goal <name>` resolves to the main repo, not a wave worktree

`load_goal`/`read_wave_config`/`wave/<name>/MEMORY.md` are all main-repo
concepts (consistent with Concerto/lfd always targeting the main repo). `lf
goal <name>` runs `main_repo_root(find_repo_root())` and launches the
orchestrator session there, regardless of which worktree you invoke it from.

## `LaunchPromptInput.wave` left unset for goal launches

Setting `wave: Some(name)` would pull `wave/<name>/README.md` and other docs
in automatically, but it also auto-injects `wave/<name>/MEMORY.md` via
`gather_wave_memory_doc` — which `render_goal`'s `<lf:wave-memory>` block
already carries explicitly. Left it unset to avoid the duplicate memory
block; the goal message is self-contained (flows, roadmap handle, metrics,
memory, in-flight).

## New `src/lfd/client.rs` duplicates `onboarding.rs`'s private HTTP helpers

`src/lfd/service/onboarding.rs` already has private `resolve_base_url`/
`auth_header`/`normalize_base_url` functions with identical logic. Didn't
refactor onboarding.rs to share the new `lfd::client` module — out of scope
for this unit and it's working code. Worth consolidating in a follow-up.

## `lf op dispatch` in-flight fetch is best-effort

`fetch_in_flight_dispatches` in `lf/commands/goal.rs` swallows all HTTP
errors and returns an empty list if `lfd` isn't reachable — the goal loop
still launches, just without in-flight context. No retry/backoff; this is a
single fetch at launch time, not a supervisor.
