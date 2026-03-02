# Review: Make every `lf ops` command mechanical

## What was implemented

Removed all LLM agent calls from `ops/`, making every ops command deterministic and offline-capable. Three ops commands previously launched agents:

- **`lf ops commit`** — now requires `-m`; fails with "message required" when omitted
- **`lf ops pr`** — now requires `--title` and `--body` (or `--refresh` for sync-only); fails with "title and body required" when omitted
- **`lf ops release`** — deprecated as orchestrator; decomposed into five mechanical commands (`release-check`, `release-notes`, `release-bump`, `release-tag`, `release-status`)

New `lf pr` step provides agent judgment for PR title/body generation, calling `lf ops pr --title --body` underneath.

Daemon's `auto_create_pr` uses mechanical titles (`"{wave_name}: draft"` or first commit subject) instead of LLM-generated ones.

## Key choices

| Decision | Why |
|----------|-----|
| `release-notes` outputs raw PR list, not prose | Mechanical ops layer; the `lf release` step adds narrative |
| `auto_create_pr` uses mechanical titles | Draft PRs get real titles when `gate` or `pr` step runs later |
| `rebase_with_recovery` returns error on conflict instead of launching agent | Rebase agent was in ops; conflict resolution now belongs to the calling step |
| `run_builtin_agent` moved to `lfd/executor/helpers.rs` | Only remaining caller is `post_step_sync` (executor resilience, not ops) |
| `resolve_wave_name` moved from `messages.rs` to `util.rs` | Still needed by `ingest`; messages.rs deleted |
| `combine_prs` no longer updates combined PR title/body | Consistent with "no agents in ops" — combined PRs get mechanical titles |

## How it fits together

```
Steps (agent judgment)          Ops (mechanical plumbing)
─────────────────────          ──────────────────────────
lf commit  ──generates msg──→  lf ops commit -m "..."
lf pr      ──generates copy─→  lf ops pr --title --body
lf release ──orchestrates───→  lf ops release-{check,notes,bump,tag}
```

The boundary is clear: ops commands never call LLM providers. They take explicit inputs and execute git/gh commands. Steps add judgment and feed results into ops.

## Deleted files

- `ops/agent.rs` — `run_builtin_agent` moved to executor
- `ops/messages.rs` — `generate_commit_message`, `generate_pr_message`, `Message` struct
- `ops/lint.rs` — `ensure_lint_passes` (lint is a step, not an ops concern)

## Risks and bottlenecks

- **`combine_prs` titles**: Combined PRs no longer get LLM-generated titles. They'll have whatever title `gh pr create` defaults to. Low risk — combined PRs are rare and the `pr` step can update later.
- **Rebase conflicts**: `rebase_with_recovery` now fails instead of launching an agent. The executor's `post_step_sync` still has agent escalation for push failures, but rebase conflicts during ops will surface as errors to the calling step.

## What's not included

- `post_step_sync`'s debug agent escalation (out of scope per design doc — executor resilience concern)
- `run_builtin_agent` removal from executor (still needed for push-failure recovery)
- Migration of `combine_prs` to use mechanical titles explicitly (it just removes the LLM call)

## Gate fix

Fixed `next.rs:74-83` where `create_or_update_pr` was called with `refresh: true, title: None, body: None` when creating a new PR. Now uses a mechanical draft title (`"{wave_name}: draft"`) matching `auto_create_pr`'s approach.
