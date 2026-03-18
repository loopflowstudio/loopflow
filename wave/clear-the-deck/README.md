# Clear the Deck

## Vision

Keep `lfd`'s deployment surface honest and small. This wave now owns the last executor decision after the deployment/auth collapse: Docker stays the blessed container path unless a measured replacement proves worth carrying. It does not own team auth, install-time CLI sugar, or iOS distribution work.

## Strategy

The deployment and auth collapse is now the baseline: docs, compose defaults, and config resolution teach two shapes (`native` and `container`) and two auth modes (`local` and `studio`). Do not reopen that matrix while finishing this wave.

The remaining pass is executor selection inside container mode. `mode` stays the deployment selector; `auth.mode` remains a tuning knob inside that shape, but `executor.sandbox` does not survive this wave. Container mode resolves to Docker. If sandbox cannot show a concrete win over Docker—or if a replacement such as Daytona cannot prove an end-to-end wave run without adding new surface area—remove it from the supported surface and shrink the code around Docker.

Do not preserve a hidden or experimental sandbox escape hatch in mainline config, runtime, docs, or compose generation. If internal experiments continue, they move off the blessed path and out of the default support story. User-facing docs, compose defaults, and installed-service guidance should tell one Docker-backed container story.

## Goals

- Docker is the only blessed container executor unless a measured replacement beats it.
- Sandbox has one explicit status instead of an adaptive half-product.
- Deploy docs, compose generation, and executor tests all describe the same support story.

## Risks

- A vague “keep both for now” outcome preserves maintenance cost without enough user value.
- A Daytona spike could look good on startup latency while still missing worktree, credential, or harness requirements.
- Partial deletion could leave stale config keys, dead executor branches, or test/doc contradictions that keep the maintenance cost alive under a smaller label.

## Metrics

- Documented deployment shapes: 2
- Blessed container executors in user-facing docs: 1
- Experimental container executors carried past this wave: 0
- Default execution paths the team supports end to end: 2
