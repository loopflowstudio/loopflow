# LOO-873 agents list: decisions and open questions

## Decided (Task comments, 2026-11-12, Jack Heart)

- "maybe we should say agents needs to be: [claude:opus, codex, claude:fable]":
  entries are `harness[:model]`; one list carries provider and model order. No
  separate model-fallback setting.
- "if you say default agent codex:sol or wahatever, that basically ADDs the
  agent to the front of the list (since an explicit harness is different tha
  ndefault harness)": a `default_agent` the list lacks is prepended; a bare
  entry and `harness:model` are different entries.
- Is a prepended entry allowed? "Only for the scope at which the default_agent
  works".
- "if your top level is : agents: [a,b,c] / default_agent: d / then the real
  order is d,a,b,c".
- "not too worried about occasionally duplicative trying of same harness": only
  an exactly equal entry is dropped behind `default_agent`.

## Open

1. `agent` and `-a` are not checked against `agents`; the list covers only what
   Loopflow picks by itself. The other reading refuses a repository
   `agent: pi` when the personal list is `[claude, codex]`.
2. A narrower `agents` replaces the wider one whole under normal precedence
   (personal over repository), so a repository list cannot narrow a personal
   one. The alternative is an intersection.
3. `supported_harnesses` (model picker only) is left as it is.
4. With an order behind it, a bare `claude` entry no longer tries Opus, Sonnet
   and Haiku by itself; the list is the only ladder. With no order (a pinned
   bare `agent: claude`, or a lone `default_agent: claude`, or `agents:
   [claude]`) the built-in model ladder still applies. Should the built-in
   ladder go entirely?
5. A launch refused on a login Loopflow does not route (the MTG-328 case,
   `account_id: null`) reports no reset time. It moves through the order once
   and then fails with the limit; it does not wait for a reset.
6. "model unavailable" was named beside usage limits as a reason to move. Only
   usage limits move a step; a model-unavailable error is not classified.
7. Status reads the move from the recorded reason's `; continuing on <agent>`
   suffix (`task_execution::MOVED_TO`); `agent_limit_recoveries` has no column
   for it. A column would need this Task's migration.
8. The old `default_agent: claude,codex` warns on every read of the file, like
   other file warnings, and `lf set default_agent claude,codex` still saves it.
   LOO-875 removes the comma reading once `agents` ships.

## Checks

- `cargo test -p loopflow --lib -- config:: agent::tests:: lf::commands::config engine::config`: pass, except `config::files::tests::relocating_wave_preferences_preserves_conflicts_and_concurrent_edits` failed once in the parallel run and passes alone.
- `cargo test -p loopflow --test integration documented_commands::`: pass.
- `cargo test -p loopflow --test integration settings_launch_tests::`: pass.
- `cargo clippy --all-targets -- -D warnings`: clean.
- Compress: `cargo test -p loopflow --lib -- agent::tests:: config:: lf::commands::config`: 140 pass; clippy clean.
- Gate: `(test(config::) | test(agent::tests::) | test(ops::task_execution::) | test(settings_launch_tests::) | test(documented_commands::) | test(configuration_changes_reach_an_existing_subscription) | test(graph_defaults_follow_task_wave_and_repository) | test(wave_setup_tests::)) | binary(two_people)`.
- Not run: a real Graph step refused by a provider limit. Demo: set
  `agents`, run a step on an exhausted Codex login, read `lf task status`.
