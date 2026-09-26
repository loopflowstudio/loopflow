# Started-work read boundary

Finish: reading filtered active Runs, projecting an existing Run's Task, and
publishing an unopened human review do not record Task execution. A real
independent Task Run records the existing Started event once and retains it after
provider exit. Reading old events does not repair or rewrite them.

Observed source: `resolve_work_selection` inserted Started while resolving
context. Read-only `runs --active --task` and Run attribution use this resolver.
The regression uses one isolated Task, actual CLI reads, a first-human standalone
Flow, and a provider stand-in; it checks persisted Task events rather than only
UI flags. Its pre-fix result and the final correction are recorded below.

Candidate: keep Work resolution read-only. Move the existing chapter-aware
Started write to actual Run capture just before provider launch, after a Run has
been created or its preparation consumed. Keep managed-worker claim behavior and
the transactional chapter check. No new registry, history scan, UI writer, or
production data repair. A declared unregistered subject cannot establish a Task
identity; only an existing registered Task receives the event.

## Result

Reproduced: the actual filtered active-Run CLI inserted one Started event before
any provider or Run was launched. `/tmp/loo291-started-before.log` fails exactly
that assertion. The corrected final CLI proof passes: filtered observation and
an unopened standalone human review leave zero Started events, including Session
list/attribution; two subsequent independent Task Runs leave exactly one event
after both providers exit. Provider children are owned stand-ins in an isolated
Home. `/tmp/loo291-started-final.log`, one test, exit 0.

The write now occurs in `begin_run_capture` after a fresh capture or consumption
of the prepared Run and before provider launch. It uses the existing synchronous
SQLite chapter transaction at this synchronous launch boundary; the now-unused
async Store wrapper was deleted. Missing/unregistered attribution has no Task to
mark. Store read/write failures remain errors. Managed worker claims retain their
own transactional Started publication. No history scan or second persistence
owner was added.

`cargo clippy --all-targets -- -D warnings` and cargo formatting pass. Final
source/artifact hashes are in `started-read-boundary-receipt.json`. The initial
after-pass preceded deletion of the unused wrapper; the final run follows it.
Review traced both interactive and headless launch through this capture owner;
the fresh CLI execution proof uses headless stand-ins. Existing historical events
were not rewritten, and this does not supply missing historical Run coverage or
configured-provider evidence for the next sidebar projection.
