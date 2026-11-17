# LOO-953 quotes and open choices

Copied from LOO-936 (`git show f80183a113:scratch/questions.md`), only what
bears on Process removal. Speaker: Jack Heart, 2026-11-16.

## What was said

- On per-kind Process tables: "do we need both?", "what are hte process tables
  for", "should that be proc.db then or something", "now that we've moved to
  having a machine server and processes dont even map 1:1 with agent sessions
  i am unconvinced something like lf top and lf ps even really makes sense".
- On "A Machine-wide view of what's running ... becomes a list of live
  Sessions and Graph runs from the server, not a Process tree": "Check".
- On bare `lf -b <skill>` runs the server does not hold: "These should
  probably register themselves in some db somewhere", then "maybe that is the
  process table all over again hmm", then "on the other hand can we just
  filter regular lf to ps?", then "go for it" (check it).
- On "rows are the source (pid and start time on the Session or Graph run),
  the OS is the check, no registry and no Process table, environment scan as
  a diagnostic": "all seems fine."
- On `outcome`: "i just wanted to flag to you as somethign that needs a better
  design"; "and like maybe lf processes and agent processes shouldnt be in the
  same table?"
- On attachment placement and Task state: "(1) on the session (2) scond"
  (attachment token and attached Process on `agent_sessions`; Graph and ship
  records carry their own end, result and error).
- On `lf history`: "what im most interested is being able to investigate
  things like tokens and time by wave or by skill or other things like that".
  LOO-936 builds that as `usage.db` turn rows.

## What the OS process list showed (2026-11-16 18:40 PST, macOS)

`lf ps --json` listed 219 Processes, all alive. `LF_STEER_SESSION` named the
Session on 46 of 46 `codex -c` and 23 of 24 `claude --name`. 21 showed no
`LF_*` variable: all 8 `lf holder` and 13 `lf` a person started. 278 OS
processes carried `LF_PROCESS_ID`, `LF_TRACE_ID` or `LF_HUMAN_SESSION`; 128 of
them were not in `lf ps` (inherited children: `browse` 50, python 24,
`codex-code-mode-host` 15, `claude` 10, `codex` 10, `uv` 4). Read from an
agent's Bash with no sandbox; a sandboxed read is untested.

## Open choices (headless calls; reversible)

See the plan's "Decisions" for the calls made at kickoff.

## Raised while writing docs

All three settled in the plan's concept review after slice 2a.

- `lf top` refreshed in place; the docs give `lf ps` no refresh mode, only
  `lf ps --subscribe --json` (a served read). Should `lf ps` keep a refreshing
  terminal view, or is `--subscribe` enough?
- Does `lf ps` list `idle` conversations held by the holder (agent alive,
  waiting for a person), or only Sessions mid-turn and Graph runs? Docs list
  them, with state `idle`.
- `lf graph show <NAME>` inspects an authored Graph today; docs make it also
  take a run id. A run id and a Graph name can't collide (UUID vs slug), but
  `lf graph runs show <RUN>` would keep the two apart. Docs use
  `lf graph show <RUN_ID>`.

## Raised while building slice 2a

Settled in the plan's concept review after slice 2a (step-first cause
order, both env variables, dev-only skew accepted).

- Failure cause order: the plan says "the Graph run's error, else its last
  failed step's error"; built the other way (step first), matching the old
  deepest-error walk so a repeated step failure still reads as one cause.
- Child adoption of a minted id: `LF_RUN_ID` (new variable) rather than
  reusing `LF_PROCESS_ID`, which still carries the parent while the journal
  records Process rows. Collapse when the journal stops recording.
- A new runner spawning an older installed lf (branch build in development):
  the old child ignores `LF_RUN_ID`, so the step row has only the spawner's
  exit code and no error, and its Session input is not found under the step id.

## Sync checks (2026-11-16)

- `cargo test -p loopflow --lib graph_tests::authored_graph_records_each_skill_as_one_session -- --exact`: passed; initial compile exposed main's Task summary reference to `graph.process`, repaired to `graph.run`. Gate/CI run suites.
- `cargo test -p loopflow-storage --lib sqlite::task_summaries::tests::an_abandoned_task_keeps_one_summary_of_its_run -- --exact`: passed with the owner-record fixture; gate/CI run suites.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings`: passed.

## Rename checks (2026-11-16)

- `cargo build -p loopflow --bin lf`, `cargo fmt`, `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test -p loopflow --lib graph_tests::authored_graph_records_each_skill_as_one_session -- --exact`: passed.
- `cargo test -p loopflow-storage --lib sqlite::task_summaries::tests::an_abandoned_task_keeps_one_summary_of_its_run -- --exact`: passed.
- `cargo test -p loopflow-storage --lib sqlite::graph_inventory::` (5) and `cargo test -p loopflow-execution --lib harness::task_updates::` (8): passed. Gate/CI run suites.
- `lf graph` now records `cwd` from the Graph's working directory and `repo` from the journal's main checkout (`journal::current_repo`), where both came from the journal context before; the two cwd values are the same path.

## Compress checks (2026-11-16)

- `cargo clippy --all-targets -- -D warnings`: passed after `RunStart::reserved` and bool `registration`.
- `cargo test -p loopflow --lib graph_tests::authored_graph_records_each_skill_as_one_session -- --exact` (1) and `task_graph_launch_tests::` (56): passed.

## QA (2026-11-16)

- `lf graph show <RUN_ID>` failed with "graph not found" though docs and `task_operate` use it; now falls back to the recorded run. Walkthrough in `scratch/qa.html`.
- Docs and operate skills already describe `lf ps` as live Sessions and Graph runs and `lf machine doctor` strays; neither is built. Land them with those slices or trim before an early merge.
