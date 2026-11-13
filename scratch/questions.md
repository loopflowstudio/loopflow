# LOO-873 agents list: decisions and open questions

## Decided (Task comments, 2026-11-12, Jack Heart)

- "maybe we should say agents needs to be: [claude:opus, codex, claude:fable]":
  entries are `harness[:model]`; one list carries provider and model order.
- "if you say default agent codex:sol or wahatever, that basically ADDs the
  agent to the front of the list (since an explicit harness is different tha
  ndefault harness)": a missing `default_agent` is prepended; a bare entry and
  `harness:model` are different entries.
- Is a prepended entry allowed? "Only for the scope at which the default_agent
  works".
- "if your top level is : agents: [a,b,c] / default_agent: d / then the real
  order is d,a,b,c".
- "not too worried about occasionally duplicative trying of same harness": only
  an exactly equal entry is dropped behind `default_agent`.
- The request spelled the list "we should just be doing agents: a,b".

## Intended outcome and current approach

One ordered `agents` list supplies harness and model choices. A headless step
refused by a usage limit tries available logins, then the remaining entries;
an exhausted order with no known account reset stops without repeating the
same Graph. A known reset retains the existing wait and recovery path. `-a`
pins one provider. The old comma spelling remains readable with a deprecation
warning.

`Config::agent_order` supplies the selected entry and fallbacks. In
`config/settings.rs::resolve_layers`, a `default_agent` leads only when its
winning declaration ranks at least as high as `agents` by (source, scope,
target). A higher list is the whole order, including over the sole installed
harness. `docs/config.md` owns the precedence explanation.

`agent/mod.rs::_run_with_transient_retries` records an exhausted limit with
`; stopped: <advice>`. `ops/task_execution.rs::limit_stop` reads it for status
and the Task retry loop. `lf/commands/graph.rs` reads `ended_step_recovery` once
for waits and stops; `TaskRunStopped` covers limit stops and repeated failures
so shipping has one halt path. An unrouted login has no reset time: the step
walks its order once, then stops. Provider reset text is not parsed.

### Manual recovery (concept review 5)

`select_task_run` now reads `ended_step_recovery`: a record with `retry_at`
or matching `limit_stop` selects that step, so earlier steps stay done. A move
followed by an unrelated failure (`; continuing on`) retains ordinary restart
behavior. The stop output, status and subscription/troubleshooting docs name
`lf task run <ID>` as the manual restart command. Automatic retries remain
suppressed and shipping stops. No new recovery type or schema change.

The concept review 5 direction is implemented in `c57f1daf3d`.
`a_run_stopped_by_a_usage_limit_is_not_started_again` checks one initial Graph,
then a manual invocation with only `graph show late`; the earlier step stays
done. `a_run_that_failed_after_moving_agents_starts_again_from_where_it_began`
checks the unrelated-failure case. `LimitWait` now carries only the reset and
agent needed by automatic restart callers.

### Remaining verification

- Verify the production recording path through a real provider refusal in
  demo. The Graph regression injects the stop record; the agent-loop tests
  script provider results. Neither establishes the whole path together.
- Gate still owns the affected suites below. The four product choices remain
  open for demo; the preceding retry-stop direction did not resolve them.

### Compatibility cleanup

LOO-875 removes the deprecated `default_agent` comma alias,
`old_default_agent_warning`, `_split_agent_list`, and their exclusive tests
once `agents` ships. Keep `config::agent_list`: comma strings are also a
supported spelling of `agents` in files, `lf set` and `--setting`.

On 2026-11-12, realign rechecked `origin/main`: `git fetch origin` still found
`78bb9f5564`, already merged in `d1184a5120`; `git log HEAD..origin/main`
was empty. The upstream Task-move change makes
`limit_recovery::restarts_by_itself` require `task_claimed_here` instead of
`task_owned`; it does not change agent ordering or the exhausted-limit stop.
`lf task status LOO-873 --json` supplied no newer product decision. No Wave is
selected. Source review confirmed that the manual-recovery code, stop/status
messages and subscription/troubleshooting docs agree; no code repair was
needed in this realign pass.

## Observed runs (2026-11-12)

Earlier detail is in `7d2dbc82fa:scratch/questions.md`.

- Throwaway home: `lf set --global agents claude,codex` wrote
  `agents: [claude, codex]`; a file `agents: codex,claude` showed `agent  codex,
  then claude` with no warning. `lf set --global default_agent claude,codex`
  printed `warning: …/config.yaml: default_agent: claude,codex is the old
  spelling; write agents: [claude, codex] and default_agent: claude`.
- Concept review 2: global `default_agent: claude` with
  `skill.implement.agents: codex` first showed `agent  claude, then codex`.
  After the rank change, `lf config settings --skill implement --explain`
  showed `agent  codex  Personal global · skill implement`, an unused
  `default_agent = claude`, and `selected` for `agents = ["codex"]`.
  Plain settings showed `agent  claude`. The deprecated spelling warned once
  per file per process (it had printed twice).
- Concept review 3: personal global `agents: [claude, codex]` with repository
  `skill.implement.default_agent: codex` showed `agent  claude, then codex
  Personal global`; over `default_agent = codex · Shared project · skill
  implement`, it said `unused: a higher agents list is the whole order`.
  The old phrase "a more specific agents list" had suggested a skill list
  that did not exist.
- The incident's config (global `default_agent: claude`,
  `skill.compress.default_agent: codex`, no `agents`) showed compress as
  `codex` alone. After `lf set --global agents claude,codex`, compress showed
  `codex, then claude`, everything else `claude, then codex`.
- Concept review 4 found the incident still repeated with no list: the agent
  failed once and `lf task run` relaunched it three times. With an exhausted
  list, each relaunch walked the whole list. The stop record now prevents
  those relaunches; the last implement pass's 152 focused tests passed.
- The routed/no-reset regression failed before the fix: `[codex, claude]`
  returned exit code 1 before Claude ran. It scripts a routed Codex usage
  refusal and unavailable accounts, both before a thread exists and after
  giving up a thread that cannot move. The no-fallback test now exercises a
  Graph step pinned to `claude:opus`.
- The focused run passed 152 tests, including the new fallback and one-Graph
  stop regressions. `relocating_wave_preferences_preserves_conflicts_and_concurrent_edits`
  failed at `finish_wave_move` with `No such file or directory`, then passed
  alone. It lacked the shared environment lock while neighboring fixtures
  replaced PATH; it now holds that lock.

Not established: why the incident's launch had `account_id: null` while
`lf account` showed four connected Codex logins, one at 84% "active and held".
An unrouted launch cannot change login, so "moves to the next login" did not
apply to it and still would not.

## Choices made without a new answer

- `-a` is not checked against `agents`: the Task says "An explicit one-off
  `-a/--agent` still pins a single provider." A configured `agent` follows
  that rule too; nobody has said so about `agent` itself.
- A narrower list replaces the wider one whole under existing precedence.
  "Narrowed or reordered per repository" is met by a personal repository
  list (`lf set agents …`); a shared repository file does not outrank a
  personal global list.
- A shipping Task stops shipping on a limit stop, as after three identical
  failures. Restarting on its backoff would walk the same exhausted order
  without a reset time to wait for.
- Only a failure classified as a usage limit records the stop. With no earlier
  limit result, no eligible account and no known reset ("failed to select
  provider account") remains an ordinary failure that can be relaunched. After
  a limit result, it advances through remaining agents before stopping.
- The agent loop cannot distinguish a pin, an absent list or a one-entry list,
  so its advice names a pin or absent list even for `agents: [codex]`. A list
  with fallback entries instead names the entries tried. That advice also
  needs to be read alongside the pin: adding `agents` does not remove `agent`
  or `-a`.
- Status reads moves from the existing `; continuing on <agent>` suffix;
  the stop uses the same record, without a migration.

## Open product choices

Each has a default built; none stops the work.

1. What does bare `claude` mean on refusal? Built: with another entry behind
   it, move straight there on its configured model; alone (`agents: [claude]`,
   lone `default_agent`, or pinned `agent: claude`), try Opus, Sonnet, Haiku.
   Thus list length changes behavior, and old `default_agent: claude,codex`
   no longer tries Sonnet and Haiku before Codex. Alternatives: always try
   the harness's models before the next entry (`claude:opus` opts out), or
   remove the built-in ladder and try only written entries. This decides
   whether `[claude, codex]` prefers Haiku or Codex after Opus.
   Task comment, 2026-11-12, Jack Heart: "if its agents: [claude, codex] /
   claude: / default_model: fable / then we need some way of establishing if
   claude:fable fails, we can still try claude:opus". The next comment: "maybe
   we should say agents needs to be: [claude:opus, codex, claude:fable]". The
   build follows the second: with `[claude, codex]` and Fable saved, a refused
   Fable goes to Codex, and Opus runs only when written
   (`[claude, claude:opus, codex]`). Nobody has said the second replaces the
   first for a bare entry.
2. `supported_harnesses` is a second list, read only by the model picker.
   Should that picker read `agents` and the setting go?
3. "model unavailable" was named beside usage limits as a reason to move.
   Only usage limits move a step.
4. Does a wider `default_agent` lead a narrower `agents`? The scope quotes
   above describe one level. Built: the narrower list is the whole order
   because the Task says "narrowed". The alternative keeps Claude, then
   Codex and makes `agent` the only way to get "Codex only". Also undecided:
   should a repository `default_agent` lead a personal list? Built: no.

Concept review 6 (2026-11-12) read the docs, config resolution, agent loop,
stop record and manual restart against the Task comments and found no design
change. Implement has no new direction.

## Checks and handoff

- Earlier checks (152 implement tests, 67 after compression, formatting and
  clippy) are recorded in `d1184a5120:scratch/questions.md`; those results
  precede the `78bb9f5564` merge. Gate still checks the combined tree.
- Implement: `cargo test -p loopflow --lib -- a_run_stopped_by_a_usage_limit a_run_that_failed_after_moving_agents a_run_that_ended_waiting a_stopped_run_is_started_again task_ship_stops_shipping status_tells_waiting_retrying_and_a_plain_failure_apart` (LF variables unset, stdin closed): four existing checks passed; both new regressions passed on focused rerun after correcting their expected argv to include `graph`. Existing linker `__eh_frame` advisory.
- Implement: `cargo test -p loopflow --test integration documented_commands::`: four passed; `cargo fmt --check` passed.
- Compress: `cargo test -p loopflow --lib -- config::settings:: agent::tests:: ops::task_execution:: a_run_stopped_by_a_usage_limit a_run_that_failed_after_moving_agents a_run_that_ended_waiting a_stopped_run_is_started_again a_restarted_wait a_shipping_tasks_restarted_wait task_ship_stops_shipping` (LF variables unset, stdin closed): 76 passed; `cargo fmt --check` and `git diff --check` passed. Existing linker `__eh_frame` advisory. Gate owns the affected suites.
- Gate: affected config, agent, task_execution, settings_launch_tests, documented_commands, configuration_changes_reach_an_existing_subscription, graph_defaults_follow_task_wave_and_repository, wave_setup_tests, task_graph_launch_tests, and two_people suites.
- QA (2026-11-12, `scratch/qa.html`): branch `lf -b graph` in a private home
  with a stream-json stand-in `claude` returning a real `is_error` usage-limit
  result. `[claude:opus, claude:sonnet]` moved Opus→Sonnet on the same thread;
  both refused stopped once with the order advice; `-a claude:opus` stopped
  after Opus; lone `claude` with Fable saved ran fable→opus→sonnet; deprecated
  comma `default_agent` and a per-skill `default_agent` led correctly. Fixed:
  moves and stops now print to stderr beside the existing wait line; doc line
  on bare-claude fallthrough. Not reached: Task path (needs a real managed
  login) and Codex on either side.
- Demo still needs a real provider refusal: the Graph test's failing step
  writes the stop record itself. With no `agents`, run on an exhausted Codex
  login, inspect the stop and `lf task status`; then set `agents` and see it
  move. No real provider refusal was run in implement or compress.
