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

## Approach and remaining cuts

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

### Delete — do not maintain

No remaining deletion targets in this pass. Separate `RepeatedRunFailure` /
`LimitStop` types and the `stopped_on_limit` wrapper are removed. Keep
`waiting_on_limit`: the sweep and restart callers need it.

LOO-875 removes the deprecated `default_agent` comma alias,
`old_default_agent_warning`, `_split_agent_list`, and their exclusive tests
once `agents` ships. Keep `config::agent_list`: comma strings are also a
supported spelling of `agents` in files, `lf set` and `--setting`.

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
- Only a failure classified as a usage limit records the stop. No eligible
  account and no known reset ("failed to select provider account") remains
  an ordinary failure that can be relaunched.
- The agent loop cannot distinguish a pin from an absent list, so its advice
  names both. An exhausted list instead names the entries tried.
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
2. `supported_harnesses` is a second list, read only by the model picker.
   Should that picker read `agents` and the setting go?
3. "model unavailable" was named beside usage limits as a reason to move.
   Only usage limits move a step.
4. Does a wider `default_agent` lead a narrower `agents`? The scope quotes
   above describe one level. Built: the narrower list is the whole order
   because the Task says "narrowed". The alternative keeps Claude, then
   Codex and makes `agent` the only way to get "Codex only". Also undecided:
   should a repository `default_agent` lead a personal list? Built: no.

## Checks and handoff

- Previous implement: `cargo test -p loopflow --lib -- config:: agent::tests:: ops::task_execution:: ops::task::limit a_run_stopped_by_a_usage_limit a_stopped_run_is_started_again task_ship_stops_shipping a_run_that_ended_waiting restarted_wait`: 152 passed; clippy clean; `documented_commands::` passed.
- Compress: `cargo test -p loopflow --lib -- config:: agent::tests:: ops::task_execution:: a_run_stopped_by_a_usage_limit a_stopped_run_is_started_again task_ship_stops_shipping a_run_that_ended_waiting restarted_wait` (with `LF_*` unset, stdin closed): 152 passed; linker reported its `__eh_frame` size advisory. Gate runs the affected suites below.
- Compress: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
- Gate: affected config, agent, task_execution, settings_launch_tests, documented_commands, configuration_changes_reach_an_existing_subscription, graph_defaults_follow_task_wave_and_repository, wave_setup_tests, task_graph_launch_tests, and two_people suites.
- Demo still needs a real provider refusal: the Graph test's failing step
  writes the stop record itself. With no `agents`, run on an exhausted Codex
  login, inspect the stop and `lf task status`; then set `agents` and see it
  move. No real provider refusal was run in implement or compress.
