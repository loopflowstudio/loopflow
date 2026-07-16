# v0.11.3

v0.11.3 makes durable Sessions survive Loopflow upgrades, tightens recovery and PR authority, and keeps the operating surfaces honest.

## Sessions resume through the installed Loopflow

Project and Task Sessions no longer pin the executable, database path, or `LF_HOME` from their first body. Every resume, supervisor wake, and handoff resolves the current Home at launch time, while each body generation records the binary provenance that actually booted it. This is the migration-fire repair: install v0.11.3 before resuming Sessions stranded on an older binary (#986).

The release adds migration `0.11.018_session_body_provenance`. Only an official release install should advance the production store.

- Body observations now reach the shared runtime wire, so surfaces can distinguish Working, Stalled, NeedsInput, Stopped, and Failed without re-deriving lifecycle state (#985).
- Explicit resume reaps dead Task and Project leases even when durable state is Waiting, Failed, or Blocked (#968, #973).
- Interactive handoff Open resolves to the last successful attach-capable surface, with an embedded fallback when that surface is gone (#969).

## Operations fail more honestly

- Task PR publication proves and heals its authoritative range end to end before GitHub receives a PR (#977).
- Read-only ops telemetry writes under ignored `.lf/tmp/` state instead of dirtying the checkout that dispatch depends on (#982).
- Ambient Wave resolution is shared by chat, radio, memory, Home, cron, and run attribution; stale UUIDs fail loudly instead of dropping work (#979).
- Creating PM work now fails closed until the Wave has an explicit Linear team binding (#975).
- Codex bodies launched by Loopflow default to standard speed; Fast remains an explicit interactive opt-in (#976).

## Evidence and Mac surfaces

- Receipt resolution keys PR evidence by repository, number, and SHA, and distinguishes inaccessible evidence from orphaned evidence (#971, #984).
- The Mac Wave/Project/Task lens and its loading, error, empty, selected, and child-indentation states are driven by shared contracts and offline proof fixtures (#963, #972).

