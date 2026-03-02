# Make every `lf ops` command mechanical

## Problem

`lf ops` commands are supposed to be pure plumbing — deterministic, no network calls to LLM providers, composable by steps. Three ops commands still launch LLM agents:

- `lf ops commit` generates commit messages via `generate_commit_message()` when `-m` is omitted
- `lf ops pr` generates PR title/body via `generate_pr_message()`
- `lf ops release` orchestrates the full release workflow including LLM-generated release notes

This means ops commands can't run offline, can't be tested without mocking agents, and blur the boundary between "mechanical operation" and "agent judgment." The `lf commit` and `lf release` steps already exist and do the right thing — they add agent judgment and call ops primitives. But the ops layer still has its own parallel agent paths.

Additionally, the daemon caller `auto_create_pr` calls `generate_pr_message()` directly rather than routing through a step.

## Approach

Remove all `launch_agent` calls from `ops/`. Delete `messages.rs`, `agent.rs`, and the release orchestrator (`publish_release`). Make every ops command fail loudly when required inputs are missing rather than silently spinning up an agent.

### Commit

`lf ops commit` without `-m` fails with exit 1: "message required — use `lf commit` to generate one."

The `lf commit` step (already exists) is the only path that generates messages. It calls `lf ops commit -m "..."`.

### PR

`lf ops pr` gains required `--title` and `--body` flags. Without them, exit 1: "title and body required — use `lf pr` to generate them."

New `lf pr` step: reads the diff, generates title/body with agent judgment, calls `lf ops pr --title "..." --body "..."`.

`lf ops pr --refresh` (no title/body) stays valid for the case where a PR already exists and just needs a rebase+push — no title update.

### Release

Delete `publish_release` from ops. The `lf release` step (already exists) is the single orchestrator. It calls decomposed primitives:

- `lf ops release-check` — exit 0 if changes, exit 1 if empty
- `lf ops release-notes <version>` — becomes mechanical: dump raw changelog (PR list as markdown), no LLM narrative. The `lf release` step adds narrative.
- `lf ops release-bump <version>` — unchanged, already mechanical
- `lf ops release-tag <version>` — unchanged, already mechanical
- `lf ops release-status` — unchanged, already mechanical

Delete `diagnose_release_failure` and `bootstrap_release` from ops. Diagnosis belongs in the step. Bootstrap belongs in `lf init`.

### Daemon callers

**`auto_create_pr`**: Stop calling `generate_pr_message()`. Use a mechanical title: `"{wave_name}: draft"` or the first commit's subject line. The PR gets a real title/body when the `gate` or `pr` step runs later in the flow.

**`post_step_sync`**: Already mechanical — uses `"lf commit: {step_name}"`. No change needed.

### Cleanup

After removing all agent calls from ops:
- Delete `ops/messages.rs` (move `Message` struct and validation to a utility if needed by steps)
- Delete `ops/agent.rs`
- Remove `OpsError::AgentFailed`
- Remove `launch_ops_agent` from release.rs
- Verify: `grep -r "launch_agent\|run_builtin_agent\|launch_ops_agent" rust/loopflow/src/ops/` returns nothing

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep messages.rs but make it optional (only call agent if available) | Graceful degradation | Blurs the boundary. If ops can sometimes generate messages, callers can't rely on the contract. |
| Move PR message gen into ops/pr.rs directly | Less files to change | Still an agent call in ops. The whole point is ops = mechanical. |
| Make auto_create_pr route through the `lf pr` step | Fully consistent | Over-engineered. Spinning up a full agent session just to write a draft PR title is wasteful. A mechanical title is fine — the step fixes it later. |

## Key decisions

1. **`lf ops pr --refresh` stays title-free.** Refresh only rebases and pushes. No title/body update. This avoids breaking the existing "just update the PR" flow.

2. **`release-notes` becomes a raw changelog, not LLM prose.** The mechanical version dumps PR titles/bodies as markdown. The `lf release` step wraps it with narrative. This means `lf ops release-notes` output looks different from the current RELEASE_NOTES.md format — but that's fine, because nobody calls it directly except the step.

3. **`auto_create_pr` uses mechanical titles.** A draft PR with title `"mobile: draft"` is better than one that requires an LLM call to exist. The flow's `gate` step will update it with a real title before the PR is marked ready.

4. **`OpsError::AgentFailed` gets deleted entirely.** After this work, no ops function should ever fail because an agent failed. If ops commands fail, it's because git failed, gh failed, or required inputs were missing.

5. **`post_step_sync`'s debug agent escalation stays.** It calls `run_builtin_agent` when push fails after rebase. This is the one remaining agent call in the daemon layer — but it's in the executor, not in ops. Removing it is out of scope; it's a daemon resilience concern, not an ops purity concern.

## Scope

- In scope: All changes to `ops/`, `lf/commands/ops/`, `lfd/executor/helpers.rs`, new `lf pr` step, test updates
- Out of scope: `post_step_sync`'s debug agent escalation, `run_builtin_agent` (used by the executor, not by ops commands), step content changes beyond creating `lf pr`

## Done when

- `lf ops commit` without `-m` returns non-zero exit with "message required"
- `lf ops pr` without `--title`/`--body` returns non-zero exit (unless `--refresh` on existing PR)
- `lf pr` step exists, generates title/body, calls `lf ops pr --title --body`
- `publish_release` deleted from `ops/release.rs`
- `diagnose_release_failure` and `bootstrap_release` removed from ops
- `release-notes` is mechanical (raw PR list as markdown)
- `ops/messages.rs` deleted
- `ops/agent.rs` deleted
- `OpsError::AgentFailed` removed from `ops/error.rs`
- `grep -r "launch_agent\|launch_ops_agent" rust/loopflow/src/ops/` returns nothing
- `cargo test --all` passes
- `cargo clippy -- -D warnings` passes
- Every `lf ops X` command works without network access to any LLM provider
