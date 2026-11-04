# Open questions and assumptions

## Settings catalogue design review — 2026-11-03

Jack Heart requested a catalogue design while the approved choice-before-fallback
pursue continues. Draft: `settings-catalog.md`. Jack chose to close appropriate
coverage gaps, not only describe them. Jack accepted the file-backed direction and
machine naming model: remove DB configuration, including machine/account preferences,
using Git-style setters. Machine aliases, identity references and connections are
config; each installation's own identity stays local. Resolve placement to an ID at
origin; a matching local ID executes locally. This replaces the earlier DB-only setter.
Registration, credentials, observations and
execution history remain separately owned; classify mixed records field by field.
Jack selected `./lf.config.local.yaml` at the repository root for private repository
settings. Implement Git ignoring and sharing across Task worktrees. Define portable
resource references, migration and file distribution explicitly. No live migration or change
to the active pursue is approved by this design discussion.

Jack clarified that openness to a personal home is conditional on solving a concrete
problem; do not introduce it as an assumed concept. The actual requirement is that
personal config set on the laptop applies to remote work, including work initiated
from mobile. Decide availability while the laptop sleeps, freshness and which devices
may edit before choosing distribution (replicated files versus a selected source
and cache). Keep declarations file-backed and preserve precedence when transported.
Explicit exclusion: shared-account servers and multi-user login servers. Paseo's
file-backed host API and identity separation remain supporting evidence.

Latest distribution candidate from Jack: the initiating machine/server pushes
personal config; the remote retains it for follow-up commands from mobile or remote
shells. This can satisfy disconnected-laptop use without a home role. Persist
authored config only, never one-command CLI/env overrides. Resolve remote editing,
conflicts and freshness next. Personal repository storage is now
`./lf.config.local.yaml`; this replaces the external repository-keyed alternatives
discussed earlier. The filename decision does not settle remote update conflicts.

Research follow-up: config-distribution-research.md recommends forwarding and
retention with refresh of unchanged copies, preserving remote edits rather than
automatically merging them. The exact retention/conflict mechanism remains proposed.
Jack accepted the per-launch authority rule: the initiating LF instance supplies
personal preferences, and the executor supplies its environment. Laptop-origin work
uses the laptop profile; phone requests through the laptop do too. Work initiated
directly on the remote uses its saved preferences. Different entry points may differ.
This neither requires automatic reverse synchronization nor authorizes overwriting
independent remote edits. Repository and choice/default resolution remain intact.

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
