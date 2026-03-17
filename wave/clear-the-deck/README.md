# Clear the Deck

## Vision

Keep `lfd`'s deployment surface honest and small. This wave now owns the last executor decision after the deployment/auth collapse: Docker stays the blessed container path unless a measured replacement proves worth carrying. It does not own team auth, install-time CLI sugar, or iOS distribution work.

## Strategy

The deployment and auth collapse is now the baseline: docs, compose defaults, and config resolution teach two shapes (`native` and `container`) and two auth modes (`local` and `studio`). Do not reopen that matrix while finishing this wave.

The remaining pass is executor selection inside container mode. `mode` stays the deployment selector; `auth.mode` and `executor.sandbox` are tuning knobs inside that shape. If sandbox cannot show a concrete win over Docker—or if a replacement such as Daytona cannot prove an end-to-end wave run without adding new surface area—cut it from the blessed story and shrink the code around Docker.

Escape hatches are still allowed, but only as clearly experimental overrides. User-facing docs, compose defaults, and installed-service guidance should keep telling one Docker-backed container story.

## Goals

- Docker is the only blessed container executor unless a measured replacement beats it.
- Sandbox has one explicit status instead of an adaptive half-product.
- Deploy docs, compose generation, and executor tests all describe the same support story.

## Risks

- A vague “keep both for now” outcome preserves maintenance cost without enough user value.
- A Daytona spike could look good on startup latency while still missing worktree, credential, or harness requirements.
- Cutting sandbox too aggressively could break internal experiments if escape hatches and tests do not move together.

## Metrics

- Documented deployment shapes: 2
- Blessed container executors in user-facing docs: 1
- Experimental container executors carried past this wave: 0
- Default execution paths the team supports end to end: 2
