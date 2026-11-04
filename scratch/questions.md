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

November 3 follow-up: Jack Heart requested documentation first, then code parity,
with aggressive conceptual cleanup. docs/config.md and docs/architecture/settings.md
are the contracts. Replace ConfigLayer's mixed source/scope/storage variants with
separate source, scope and target throughout code, DTOs and inspection. Delete
obsolete authorities and parallel paths, not just misleading names.

Jack resolved the earlier stop: private preferences, including account order, are
per independent clone; linked Task worktrees share. Legacy origin-wide routes may
be materialized into identified existing clones after collision checks. Preserve
unavailable destinations as non-executing recovery exports for explicit import;
future clones do not inherit automatically. This intentionally changes scope and
removes the earlier migration contradiction. Implementation can resume after the
docs-first checkpoint, keeping Auto-Ship stopped and limit recovery separate.

## Duplicate run evidence — November 2

A second Graph accidentally launched by `lf graph code` was stopped without
edits. The incident and process IDs remain at
`a7a64269c6:scratch/questions.md`. `lf help graph <name>` inspects a definition;
`lf graph <name>` executes it. Preserve the current Graph's ownership.

## Resume fallback (November 3)

Assumption for Jack Heart's choice-before-fallback revision: a CLI/env default_agent
is not an explicit cross-provider import request on resume. Keep native history's
provider, applying the fallback model when it names that same provider. Explicit
agent choices retain the existing switch behavior.

## Reversible implementation choices — November 3

Private Wave selectors use authored UUIDs; CLI names resolve to those IDs. Setting
`machine` names a Machine for Tasks and Sessions; capacity uses `machine.capacity`.
On-demand declarations created through a generic file edit default to zero permitted
machines and a zero hourly limit until explicitly configured. Pool availability,
rental templates/limits and authentication browser selection are file preferences;
observed browser identities and acquired-machine commitments remain runtime facts.

Configuration observation: reuse the watch reader's one-second tick to compare
the files actually read, including missing paths. This avoids platform-specific
file event loss and inode replacement races; expensive Work reads retain their
existing coalescing and rest policy. Bounded Task workers inherit the observation.

Destination edit assumption (November 3): preserve Appearance's configured-file
transaction as a fixed `theme` operation through the common writer. The ordinary
editor accepts only typed destinations; no file/source/scope assertions remain.
The removed `config file` command was introduced on this branch and has no
documented external compatibility obligation.

Remote definitions (November 3): preserve personal skill/Graph bodies and topology
on the executor. Retention may edit LF preferences in existing definitions only;
a missing definition is reported for installation/reconciliation, not synthesized.
Unnamed occurrence metadata uses its defining Graph's authored position; named
occurrences use their authored ID. These are addresses, not additional precedence.
