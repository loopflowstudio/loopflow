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

   Argument for module: simpler dependency graph, single crate to maintain.

   Argument for crate: cleaner separation, workflow could depend on engine but engine doesn't know about workflow.

   **Assumption made:** Module in `loopflow-engine` (simpler).

2. **How to handle interactive confirmation in workflow functions?**

   The `Progress` trait has `confirm(&self, msg: &str) -> bool`. For CLI this prompts stdin. For lfd, this should probably fail or use a config flag like `--yes`.

   **Assumption made:** Progress trait handles it; implementations differ.

### Agent Integration

3. **Should agent-generated commit messages use a step file or inline prompt?**

   Python has a `commit` step in `.lf/steps/commit.md`. The design assumes we load this step.

   **Assumption made:** Use step files. If missing, fall back to inline prompt.

4. **What happens if agent fails during lint fix or conflict resolution?**

   Options: retry, abort workflow, prompt user.

   **Assumption made:** Abort workflow with error. User can retry manually.

### Parity Gaps

5. **Should we support stacked PRs?**

   Python detects if current branch has an open PR and stacks. The existing gap analysis mentions this.

   **Assumption made:** In scope. Use `gh pr list --head BRANCH` to detect.

6. **Should `lf ops next --block` poll for merge or use webhooks?**

   Polling is simpler but wastes API calls. Webhooks require lfd to be running.

   **Assumption made:** Polling with exponential backoff. `--block` is CLI-only anyway.
