# Implementation decisions and assumptions

## Settings catalogue extension — November 3, 2026

Jack Heart authorized implementation with “ok. lets build.” The current plan is
settings-catalog.md. This replaces the earlier draft-only exclusions; continue in
LOO-714 and PR #1805, using Codex, publishing for review with Auto-Ship stopped.
Automatic failure recovery remains the later PR. Preserve unrelated critique files.

Accepted: one catalogue, coverage of every appropriate setting surface, file-backed
preferences and setters, no DB preference override, root ./lf.config.local.yaml,
choice-before-fallback, origin personal preferences for remote requests, executor
host facts, saved remote preferences for directly initiated work, and configuration
templates versus DB live layouts. No per-skill accounts or built-in agent recommendations.

Routine choices for the authorized implementation are recorded in the plan: share
one main-checkout private file across worktrees, ignore it via common info/exclude,
atomically edit files, retain origin declarations and refresh only unchanged copies.
Preserve independent remote edits and make persistence status explicit without
blocking forwarded work. No automatic reverse sync, Home role or multi-writer merge.
Record any architectural counterexample here rather than silently weakening the
accepted outcome. Remote profile differences are deliberate per-launch authority;
existing runtime account/thread continuity still applies.

Unresolved authority conflict (November 3 implementation): repository account
routes currently follow Git `owner/repo` across independent and future clones,
while the accepted private file belongs to one main checkout. Preserving that
scope and deleting every identity-wide preference owner cannot both hold.
`settings-catalog.md` records the source evidence, alternative design decisions,
preserved prototype and remaining work. No scope revision is attributed to Jack
Heart; dependent implementation stopped and pre-run code was restored.

## Duplicate implement run stopped (2026-11-02 14:25)

A second `code` Graph (process 768383a6, `lf graph code`) was launched in this
worktree by an exploratory shell command from the primary implement agent
(`lf help graph; lf graph code | head -40`). Its implement step found two
implement agents on one checkout, made no code changes, and stopped its own
Graph (PID 83352) so it would not run compress/sync/pr-publish against the
primary run's live edits. The primary run (Graph f6f522e2, `lf task run
LOO-714`) owns this step. `lf help graph <name>` explains a graph without
launching it; `lf graph <name>` runs it.

## Resume fallback (November 3)

Assumption for Jack Heart's choice-before-fallback revision: a CLI/env default_agent
is not an explicit cross-provider import request on resume. Keep native history's
provider, applying the fallback model when it names that same provider. Explicit
agent choices retain the existing switch behavior.
