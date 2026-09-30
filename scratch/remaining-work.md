# Remaining work to code-complete review

LOO-298 · Realigned 2026-09-30 for Jack Heart. Order follows Jack's direction
"do the deepest cuts first." Work top down. Decisions and their wording are in
[questions](questions.md); the accepted model is
[data-model-one-table-per.md](data-model-one-table-per.md).

## Model now in force

- **Exec** is one actual lf process. Every Flow step is its own Exec: skills,
  routers and reviews run the ordinary `lf skill` command; ops run their own
  command. An agent-issued lf command's parent is the lf process driving that
  agent (Jack, 2026-09-29).
- **AgentSession** is one conversation. A captured input is an event in its
  history that names its Exec; imported history with no process keeps none.
- **FlowSession** is one started Flow. A definition may reference other Flows;
  starting it compiles them into one graph of skills and ops. Subflows and loop
  passes are lenses over that graph and its history, never their own
  FlowSessions (Jack, 2026-09-29 and 2026-09-30).
- **Flow decisions** are typed results of the selected successful turn. The
  in-turn decide, route and blocked commands are gone. Blocked requires a reason
  and returns Ask feedback to another turn of the same conversation.
- **Where running a skill directly and running it as a Task step disagree,
  the direct behavior wins** (Jack). Task specifics are input to the same command.

## Working rhythm (Jack, 2026-09-30)

One item per implement iteration, then publish and let the loop decide. Prove
each with focused tests for the changed behavior and hosted CI; run the full
local Rust suite only before the final gate. New decisions arrive at implement
boundaries.

## Reconciled baseline

Rebased onto main `a6b1bc3df`; reconciliation `27e4d776e` preserves typed CLI
command discovery, precedence and flattened step settings. Saved
`flow list/show --sessions` reaches SQL; selected steps execute captured Skills
before mutable catalog lookup; Ask escapes reserved Skill names. The short guide
is `docs/lf.md`, with detailed execution contracts in `docs/lf-reference.md`.
[Rebase evidence](rebase-main.md) records 13 focused passes and static checks.
No new full-suite or configured acceptance result follows from this realignment.

## Order, deepest first

1. **Every step runs its ordinary lf command.** Ops and skills are converted and
   both launchers are deleted (`9cb7b4c86`, net −1,242). Common Task seed/name,
   checkout context after landing, PATH step selection and schema-gap preservation
   are checkpointed at `2ebfd9f51`. Public scripted account/name/context,
   driver recovery and Chapter preservation passed. Retain managed repeated
   decisions, failure release, explicit retry and configured account/control
   proofs. Task controls already use the common command;
   do not build another transport. Typed `blocked` with a required reason now
   replaces `lf flow blocked`; the public taskless proof recovers the keyed Ask
   and continues three deciding turns in the same native conversation. See
   [blocked-decision evidence](evidence.md#structured-blocked-decisions--2026-09-30);
   managed/interactive acceptance is still distinct. See
   [Task-command evidence](evidence.md#task-command-slice-review--2026-09-30) and
   [the before/after inventory](exec-per-step.md#direct-skill-command-cut--2026-09-29).
2. **Remove loop passes as FlowSessions — done.** Implementation `a66f42a0a`
   replayed as `cbd2b1d7c`; checkpoint simplification is `4f2ec5491`. One
   FlowSession retains its cursor and return counters across retry and Iterate.
   The new direct ownership migration creates that final shape. The earlier
   child-pass archive and intermediate migration are deleted per Jack's current
   compression decision; loop/retry behavior and configured acceptance remain.
3. **Simplify attribution and the parent tree — done.** Implementation
   `974f1dfd5` and fixture simplification `7beb3600b` complete this item.
   The final Exec schema omits the dead caller token. No intermediate table or
   archive survives.
   Native test helpers reference the existing Session event sequence, without
   synthetic caller Execs. Session/provider-generation parent resolution remains.
   Explicit command `--as` → checkout → inherited `LF_AS` is unchanged.
   The rebase repair uses that same binding reader when deciding whether a skill
   may checkpoint shared edits; main's `skill -- NAME` and `cmd:` grammar stays.
   [Evidence](evidence.md#caller-token-removal-and-rebase-repairs--2026-09-30)
   records the requested file suites, source/materialized migration, real Codex
   with synthetic Responses, 18 Swift DTO checks and 13 compression checks.
   The Docker installation check remains unexecuted: its ten-second probe timed
   out before creating a container (`caller-install.log`). These local proofs
   do not close configured acceptance, broader Flow membership provenance or
   current-state cutover.
4. **One naming commit — implemented.** The [before/after table](naming.md)
   records Exec process APIs, Session capture/recorder types and `compile_flow`.
   Manifest and saved-graph encodings now use the current names only. The real
   Desktop breadcrumb consumer retains definition provenance as `sources` in
   Rust/Swift/current wire fixtures. `session_events` → `agent_events` and
   the `run_events` ledger has been deleted. Focused proof and measured
   production changes are in [evidence](evidence.md). Jack's acceptance remains
   separate; no new lifecycle owner is introduced.
5. **Compress historical compatibility — implemented.** Jack Heart superseded historical
   import and intermediate-draft preservation on 2026-09-30: “Whatever history
   or extra state we don't need, toss it now.” Retain current operating state
   and resumable conversations; remove finished-history import, old formats and
   archives. Inventory: [compress-history.md](compress-history.md). Verify a
   fresh Home and a copy of the installed database; never migrate the original.
6. **Docs, skills and generated pages** for final behavior after the remaining
   cuts. The architecture reference already records the successful Codex
   synthetic-Responses retry and the one-FlowSession implementation; retain
   configured-provider limits. Keep main's short guide/reference split.

Done and verified on hosted CI: structured-result decisions, captured input as a
Session event, typed history readers, saved-Flow discovery with Desktop paging,
worker claim handoff, mechanical steps as child Execs.

## Obligations that still bind

These are not implementation items and are not closed by fixtures:

- **Configured acceptance.** Real accounts for Codex, Claude and OpenCode;
  interactive reconnect; a configured Desktop run.
- **Dense measurements.** Cold and warm CLI list and detail on a representative
  store, with startup, SQL and payload separated.
- **Chapters.** Remaining proofs in [chapters.md](chapters.md).
- **Incident.** Cancellation with a live owned child still needs a settlement
  proof; causal ancestry never authorizes a signal.
- **Real-Home conversion and release.** Backup, rehearsal, quiescence and
  separate authority. No branch binary touches an installed Home.
- **Integrated finish.** Full affected suites once on final bytes, migration and
  architecture checks, formatting and all-target Clippy, then the saved Flow's
  delivery steps.

The superseded checklist and its receipts are in history at
`d61295196:scratch/remaining-work.md`.
