# 5 Whys: PR #396 merged with garbage title

## The Problem

PR #396 (24 commits of Concerto NUX + agent harness work) merged to main with the title "Updated PR #396: https://github.com/loopflowstudio/loopflow/pull/396" and an empty body. This self-referential string is now the squash merge commit message on main.

## Chain

Problem → LLM-generated title used as merge commit → No validation on LLM output → Fallback parser accepts anything → Agent has full tool access for a text-generation task → No human gate before irreversible merge

**Problem**: Squash merge commit on main reads `Updated PR #396: https://github.com/loopflowstudio/loopflow/pull/396 (#396)` — meaningless for a branch that shipped two major Concerto features.

**Why 1**: `enable_auto_merge()` in `land.rs:234` passes the LLM-generated title directly as the `--subject` for `gh pr merge --squash --auto`. The squash merge commit inherits whatever the LLM produced.
↳ *Could we have caught this earlier?* Yes — the title should be validated before being passed to an irreversible operation.

**Why 2**: `finalize_remote()` in `land.rs:128` calls `generate_pr_message()` and pipes the result straight to `update_pr_message()` and `enable_auto_merge()` with no validation. The `Message { title, body }` struct is trusted unconditionally.
↳ *What process allowed this?* The land flow treats LLM output as infallible. There's no quality check between generation and use.

**Why 3**: `generate_pr_message()` in `messages.rs:92` launched a full Claude Code session (`claude --print --dangerously-skip-permissions`) to generate a title. The agent likely interpreted "Generate a PR title and body" as an action ("update the PR") rather than a text-generation task, and returned a status message like "Updated PR #396: URL" instead of JSON. The `parse_message_output` fallback (line 162) treated this first line as the title and empty remainder as the body.
↳ *What assumption was wrong?* That a full coding agent (with bash, git, and gh access) would reliably produce structured JSON when asked for text generation. The agent may have run `gh` commands itself.

**Why 4**: The agent is launched with `--dangerously-skip-permissions` and `auto: true` for what is fundamentally a structured text extraction task. It has access to every tool Claude Code offers — bash, file I/O, git, gh — when all it needs is to read a diff and return two strings. The excessive capability surface makes the output unpredictable.
↳ *Why was that assumption encoded?* `generate_message()` reuses the general `launch_agent` infrastructure. There's no "text-only" agent mode — every agent invocation gets the full tool suite.

**Why 5 (Root)**: Message generation conflates two concerns: (1) understanding the diff (which benefits from code intelligence) and (2) producing structured output (which needs constrained output, not tool access). By using a general-purpose agent for both, the system gets neither reliability nor validation. The fallback parser compounds this by accepting any output shape, and the land flow compounds it further by using the result for an irreversible merge commit without review.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 3 | What exactly did the agent do? No logs of the Claude session output are retained. | High |
| Why 3 | Was the diff too large and the agent timed out or hit context limits? | Medium |
| Why 4 | Would a simpler agent (API call, not CLI) be more reliable for structured output? | High |
| Why 1 | Should `enable_auto_merge` require explicit human confirmation of the merge title? | Medium |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Reword the squash merge commit on main (if repo policy allows) or accept the bad commit message | This specific instance |
| Structural | Validate `Message` output — reject titles containing URLs, titles > 100 chars, empty bodies for large diffs, self-referential patterns | This class of bad LLM output |
| Structural | Log the raw agent output from message generation so failures can be diagnosed | Blindness to what went wrong |
| Systemic | Add a confirmation step in `finalize_remote` that shows the generated title/body before writing to GitHub and enabling auto-merge (interactive mode only) | LLM garbage reaching irreversible operations |

## Changes to Implement

- [ ] Add `Message::validate()` that rejects titles containing URLs, titles over 100 chars, and empty bodies when the diff is non-trivial
- [ ] Make `parse_message_output` return `Err` instead of falling back to first-line-as-title when no JSON is found
- [ ] Log raw agent stdout/stderr from `generate_message()` to a file under `.lf/logs/` for post-mortem debugging
- [ ] (Longer term) Add interactive confirmation of merge title in `finalize_remote` when not in auto mode
