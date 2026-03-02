# 06: Commit message + PR message extraction

**Finish line:** `lf ops commit` and `lf ops pr` are fully mechanical — no LLM calls. Message generation lives in steps.

## Commit

`lf ops commit` without `-m` fails with "message required." The `lf commit` step generates a message and calls `lf ops commit -m "..."`.

## PR

`lf ops pr` requires `--title` and `--body` flags (or `--draft --fill` for draft PRs from commit messages). New `lf pr` step reads the diff, writes title/body, calls `lf ops pr --title "..." --body "..."`.

## Daemon callers

`auto_create_pr` and `post_step_sync` route through steps instead of calling ops directly when they need LLM-generated messages.

Note: as of PR 1, `post_step_sync` already fails on rebase conflicts instead of silently resolving them via agent. This sprint extends that pattern — daemon callers that need LLM-generated messages should route through steps, not call ops directly. Watch for new daemon errors during the transition.

## Cleanup

`OpsError::AgentFailed` is still used by `messages.rs`, `agent.rs`, and `release.rs`. After this sprint removes the message-generation agents, review whether `AgentFailed` can be removed (depends on sprint 07 for release).

## Done when

- `lf ops commit` without `-m` returns a non-zero exit with "message required"
- `lf ops pr` without `--title`/`--body` returns a non-zero exit
- `lf pr` exists as a step that generates title/body and calls ops
- Daemon callers use steps for message generation
