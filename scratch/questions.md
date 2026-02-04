# Open Questions

Questions that emerged during lfd-primary implementation. Need resolution before or during next iteration.

## Architecture

- **RunWave overrides**: `flow`, `direction`, `area` overrides currently persist to `Wave`. Design says wave config shouldn't change. Should we add override fields to `WaveRun` instead?

## Scheduler slots

- **Watch/cron bypass slots**: Loop ticker acquires scheduler slots before creating WaveRun, but watch/cron triggers start runs without slot checks. Should they also acquire/release scheduler slots?
- **Interactive resumes skip slot checks**: ConnectWave/EndAgent resumes call executor directly without acquiring scheduler slots. Should interactive resumes also acquire/release slots?

## Deferred features

- **Choose/Fork selection**: `ForkSelect::One` picks first option deterministically (no LLM). `ForkSelect::Prompt` also picks first. When should we wire a choice agent?
- **Fork retry tracking**: Design has `fork_attempts` placeholder but it's not implemented. Add when we hit transient failures in practice.

## WaveService Protocol

- Should `/wave-runs` move to a `/v1/` endpoint or keep legacy-style `ok/result` responses long-term?
- Should `WaveServiceFactory` select contexts from config (grpc/remote) instead of always returning `LocalWaveService`?

## Concerto Wave Phase Status

**Observation**: The Phase 1 ordered set in `roadmap/concerto/README.md` references items that don't exist:
- `20260131-02-history-and-recency.md`
- `20260131-03-waiting-state-actionable.md`
- `20260131-04-running-state-progress-and-connect.md`
- `20260131-05-empty-state-creates-and-teaches.md`
- `20260131-06-quick-experiment-path.md`

Recent commits suggest items 04 and 06 have shipped. Item 01 is noted as complete in the README.

**Question**: Is Phase 1 complete? Should we:
1. Move to Phase 2 (Remote access foundation)?
2. Update the README to reflect Phase 1 completion?
3. Create the missing Phase 1 items if they're not actually done?

The remaining items in `roadmap/concerto/` are all Phase 2 or 3.

## Auth Implementation

- Refresh endpoint response: assumed /auth/refresh returns JSON {token|jwt} or raw token string; confirm actual contract.
- Package supports macOS 15 only; design doc targets macOS 14+/iOS 17+. If iOS support is required, update Swift package platforms and add iOS app target.
- RemoteWaveService not present in repo; TokenProvider added but not wired into any remote service yet.

## lf ops Rust-Native Workflow Engine

### Architecture

1. **Should `workflow` be a separate crate or module in `loopflow-engine`?**

   **Resolved:** Separate crate named `loopflow-ops`. Cleaner separation—ops depends on engine, engine doesn't know about ops. The name maps directly to `lf ops` commands.

2. **How to handle interactive confirmation in workflow functions?**

   **Resolved:** `Progress` trait with `confirm(&self, msg: &str) -> bool`. CLI prompts stdin; lfd uses config flags or fails.

### Agent Integration

3. **Should agent-generated commit messages use a step file or inline prompt?**

   **Resolved:** No step file. Agent runs in batch mode with diff context, returns JSON `{title, body}`. Commit message style comes from CLAUDE.md (already in agent context). lf does `git commit` with the generated message.

4. **What happens if agent fails during lint fix or conflict resolution?**

   **Resolved:** Abort workflow with error. User can retry manually.

5. **Should agent work be batched?**

   **Resolved:** Yes. Pre-check all tasks (needs commit? needs conflict resolution?), then launch 0 or 1 agent call per operation with combined task list. Mechanical operations (clear scratch, push, pr ready, auto-merge) don't involve agent.

### Commit Format

6. **What format for commit messages?**

   **Resolved:** `lf {flow_parents} {task}: {generated_title}` with optional body. flow_parents tracks the wave/flow hierarchy (ancestors, not siblings). Examples:
   - `lf commit: add dark mode` (direct ops command)
   - `lf my-wave ship implement: refactor auth` (step in flow in wave)

### Parity Gaps

7. **Should we support stacked PRs?**

   **Resolved:** In scope. Use `gh pr list --head BRANCH` to detect.

8. **Should `lf ops next --block` poll for merge or use webhooks?**

   **Resolved:** Polling with exponential backoff. `--block` is CLI-only anyway.

## loopflow-ops implementation notes (2026-02-04)

1. **Rebase assistant prompt**: Rust ops uses a custom inline prompt instead of a dedicated `rebase` step file. There is no Rust builtin `rebase` step today. Should we add builtin ops steps to `loopflow-engine` and switch to them?
2. **Next branch naming**: `lf ops next` in Rust still uses `next-<timestamp>` instead of wave-based branch naming (no wave metadata update in Rust yet). Should we wire it to wave naming once the wave module is ported?
3. **Land local strategy**: `lf ops land --local` uses squash merge via `git::land` and skips PR-related steps. Confirm if we need a local merge option or to support `--strategy` explicitly.
4. **Lint fixer**: lint retry uses the built-in `lint` step via prompt context, not `lf lint -b` subprocess. Is that preferred for parity?
