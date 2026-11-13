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

- Demo still needs real managed-login, Codex and Task-path refusal checks.
  QA exercised Claude refusals through the production Graph recording path
  using a stream-json stand-in (`scratch/qa.html`); the Task regression
  injects its stop record. Real-provider checks have not passed.
- The affected suites, clippy, architecture, inventory, website and container-only
  cross-Task agent check passed in the publication pass below. The four product
  choices remain open for demo. The gate follow-up below repaired two further
  fallback transitions; its final agent/Task rerun passed 113 tests.

### Compatibility cleanup

LOO-875 removes the deprecated `default_agent` comma alias,
`old_default_agent_warning`, `_split_agent_list`, and their exclusive tests
once `agents` ships. Keep `config::agent_list`: comma strings are also a
supported spelling of `agents` in files, `lf set` and `--setting`.

### Stop advice

`dfd8d74ed4` corrects `_limit_stop_advice`: change or remove an `agent`/`-a`
pin separately from adding fallback entries to a missing or one-entry list.
The existing no-fallback and exhausted-order branches remain; no configuration
plumbing was added. The focused advice regression and `docs/troubleshooting.md`
use the same distinction. The preceding feedback's wording action is complete.

On 2026-11-12, realign rechecked `origin/main`: `git fetch origin` still found
`78bb9f5564`, already merged in `d1184a5120`; `git log HEAD..origin/main`
was empty. The upstream Task-move change makes
`limit_recovery::restarts_by_itself` require `task_claimed_here` instead of
`task_owned`; it does not change agent ordering or the exhausted-limit stop.
`lf task status LOO-873 --json` supplied no newer product decision. No Wave is
selected. After `dfd8d74ed4`, realign repeated the upstream and Task-comment
reads: the same upstream tip and no newer product decision. The advice, focused
regression and troubleshooting docs agree; no code repair or test rerun was
needed at that point. The gate follow-up below completes automated verification;
the real-provider demo checks above remain outstanding.

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
  so its advice covers each condition: change the pin, or remove it and
  configure fallback entries; without a pin, configure fallbacks for a missing
  or one-entry list. A list with fallback entries names the entries tried.
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

Concept review 7 (2026-11-12) read `8dbd1d9405` and `dfd8d74ed4`: the
`Continuing on <agent>: <reason>` and `Stopped: <advice>` stderr lines sit on
the same paths as the existing `Waiting for <provider> capacity` line and say
what the stop record and `lf task status` say; the advice and
`docs/troubleshooting.md` separate a pin from a short list the same way. Task
comments end at 22:12Z with the quotes above. No design change; the four
choices stay open for demo with their built defaults.

## Checks and handoff

- Gate follow-up plan (2026-11-13): config/catalog/settings/file edits reach
  `config::`, `settings_launch_tests::`, configuration subscription, Graph
  defaults, `wave_setup_tests::` and `two_people`; agent/recovery/Graph/shipping
  edits reach `agent::`, `ops::task_execution::` and `task_graph_launch_tests::`.
  Docs and repository QA skill reach `documented_commands::` and website;
  architecture, Rust inventory, fmt and clippy complete the plan. No embedded
  builtin, protocol, schema or Desktop files changed.
- Gate runner `scripts/test.py --reuse-passing --rust-filter <above suites>`:
  resource preflight failed before tests; uv cache inventory exceeded 15 seconds
  with 92.6 GiB free. The attempted cleanup exited 2. Running the printed plan
  directly per `scripts/testing.md`; no product check is counted from that run.
- `rustup update stable --no-self-update`: current stable is Rust 1.100.0;
  the direct checks use rustup ahead of Homebrew's Rust 1.98.1.
- Gate found two fallback transitions: a refused launch followed by an
  unavailable fallback lost its saved limit and aborted before later entries;
  a wait starting on a fallback reset the list index to zero while keeping
  that fallback's launch, causing an extra attempt after its next refusal.
  The loop now retains the failure across moves and stores the wait's starting
  index with its launch. Both new regressions failed before their repairs:
  `an_unavailable_fallback_keeps_walking_after_a_usage_refusal` returned
  `no eligible managed claude account`, and
  `a_fallback_wait_keeps_its_place_in_the_order` made a fourth launch after
  the three scripted attempts.
- Gate direct plan, after the first repair: `cargo nextest run -p loopflow
  --no-tests fail --no-fail-fast --build-jobs 4 --test-threads 4 -E
  'test(config::) | test(agent::) | test(ops::task_execution::) |
  test(settings_launch_tests::) | test(documented_commands::) |
  test(configuration_changes_reach_an_existing_subscription) |
  test(graph_defaults_follow_task_wave_and_repository) | test(wave_setup_tests::) |
  test(task_graph_launch_tests::) | binary(two_people)'` through
  `scripts/test_network.py`, with a prepared CLI and LF variables unset:
  237 passed in 155.757 s; one leaky output pipe in
  `agent::session::tests::bare_harness_sends_no_model_and_a_pinned_one_sends_it`.
  A surviving fixture holder exited by the follow-up process check without a
  signal. After the second repair, only agent/task suites need another pass.
- Gate: `cargo fmt --all -- --check`, `uv run python scripts/check_architecture.py`
  and `uv run --no-sync python scripts/check_rust_test_inventory.py` passed.
- Gate final repair: the same isolated nextest command narrowed to
  `test(agent::) | test(ops::task_execution::) | test(task_graph_launch_tests::)`:
  113 passed in 94.112 s, including both regressions, no leak warning. The
  remaining config/docs suites retain the preceding pass. Fixture holders
  exited without signals; no candidate `lf` processes remained afterward.
- Gate: `cargo clippy --all-targets --jobs 4 -- -D warnings` passed on Rust
  1.100.0. Cargo emitted manifest advisories for unchanged `chrono-tz`,
  `rpassword` and `tokio-stream` dependencies; no Rust lint failures.
- Gate: `cd website && uv run python dev.py test` with the candidate CLI first
  on PATH and LF variables unset: 73 passed in 24.12 s, including rendered
  config/subscription guides and accessibility. `git diff --check` passed.
  Full-matrix CI and real-provider demo remain separate; no new check is
  deferred from this gate's affected plan.

- Headless demo decision pass 1: no hands-on demo happened and no human feedback
  was collected. Advance on the recorded QA and publication results: the
  intended terminal move/stop advice is visible, the later Task-path QA fixed
  repeated capacity messages and observed recovery through Graph completion,
  and the affected publication suites passed. Real Codex refusal and
  login-to-login failover remain unobserved; that missing coverage does not
  establish a repairable implementation defect. Keep the four documented
  product defaults for this pass, without treating them as Jack's approval.

- Publication (2026-11-13), after `lf sync` left base `78bb9f5564` unchanged: the gate runner stopped before tests because `~/.cache/uv` inventory exceeded 15 seconds (92.9 GiB free). Ran its planned commands directly per `scripts/testing.md`: affected nextest filter (`config::`, `agent::`, `ops::task_execution::`, settings launch, documented commands, configuration subscription, graph defaults, Wave setup, Task Graph launch, `two_people`) 236 passed; `cargo clippy --all-targets --jobs 4 -- -D warnings`, formatting, architecture and Rust test inventory passed. Website with candidate CLI: 73 passed. Existing linker `__eh_frame` advisory.
- Publication: `uv run python scripts/test_task_installation.py --test declared_agent_can_start_another_tasks_graph` failed before tests: Linux `rustc` compiling the library test received SIGKILL with two build jobs. The same script run in memory with `CARGO_BUILD_JOBS=1` passed the selected check and both prerequisite migration checks; repository source was unchanged and both disposable containers were removed.
- Advice correction and compression: `cargo test -p loopflow --lib agent::tests::a_pinned_agent_refused_at_launch_fails_with_its_limit` (LF variables unset, stdin closed): 1 passed after the shorter wording; existing linker `__eh_frame` advisory. `cargo fmt --check` and `git diff --check` passed. Gate owns affected suites and clippy.
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
- QA second pass (2026-11-13, `scratch/qa.html`): real `lf -b task run` with a
  fixture managed login. A capacity wait reprinted every move on each 60 s poll
  and replaced the wait in `lf task status`; `CapacityWait::fall_through` now
  announces moves only before waiting (`agent::tests::` 45 passed). A Sonnet
  refusal cools the only login, so `[claude:sonnet, claude:opus, …]` waits
  instead of moving or stopping; the Task-path stop stays covered only by the
  regression.
- Demo still needs a real provider refusal: the Graph test's failing step
  writes the stop record itself. With no `agents`, run on an exhausted Codex
  login, inspect the stop and `lf task status`; then set `agents` and see it
  move. No real provider refusal was run in implement or compress.
