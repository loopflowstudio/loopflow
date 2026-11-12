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

## Built from the concept review (2026-11-12)

The request spelled the list "we should just be doing agents: a,b".

- `agents` reads a comma string as the list in files, `lf set` and
  `--setting`; `lf set` saves it as a YAML list. `config::agent_list` splits
  it, so LOO-875 deletes `old_default_agent_warning` and `_split_agent_list`
  (`config/settings.rs`) but keeps `agent_list`.
- `lf set … default_agent claude,codex` warns with the replacement as it saves.
- `docs/config.md` says a lone bare `claude` tries Opus, Sonnet, Haiku; which
  rule is right is still open choice 1.

Run in a throwaway home: `lf set --global agents claude,codex` wrote
`agents: [claude, codex]`; a file `agents: codex,claude` showed `agent  codex,
then claude` with no warning; `lf set --global default_agent claude,codex`
printed `warning: …/config.yaml: default_agent: claude,codex is the old
spelling; write agents: [claude, codex] and default_agent: claude`.

## Next implement direction (concept review 2, 2026-11-12)

A narrower `agents` does not narrow when a wider `default_agent` exists. Run in
a throwaway home with `default_agent: claude` (global) and
`lf set --global skill.implement.agents codex`:

    agent    claude, then codex  Personal global · skill implement + Personal global

Implement starts on Claude. `docs/config.md` says of the same shape
(`lf set --global skill.review.agents codex`) "review: Codex only, everywhere",
and of `lf --wave frontend set agents claude` "this Wave: Claude only". Both
are false for the config in the Task's incident, which has a global
`default_agent: claude`. The Task asks for a list "set globally and narrowed or
reordered per repository, Wave or skill".

Build (default for open choice 4):

- `default_agent` leads the order only when it is declared at least as
  specifically as the `agents` in force, by the source, scope and target
  ordering both keys already use. A more specific `agents` is the whole order.
  Same level keeps "d,a,b,c"; `skill.compress.default_agent: codex` over a
  global list keeps "Codex, then Claude".
- `--explain` marks the outranked `default_agent` candidate with why, as it
  does for a list under an `agent` choice.
- `docs/config.md`: one row in the "Keep going" table and one in the
  precedence table for global `default_agent: claude` plus
  `skill.review.agents: [codex]` → Codex only.
- Test in `agents_orders_what_a_fallback_moves_through`: that shape, and the
  Wave-scoped list under a global `default_agent`.
- The old-spelling warning printed twice for one `lf config settings` read of
  `default_agent: claude,codex`; print it once per file.

## Settled without a new answer

- `-a` is not checked against `agents`: the Task says "An explicit one-off
  `-a/--agent` still pins a single provider." A configured `agent` follows the
  same rule because the style guide prefers dropping a refusal to adding an
  allowlist; nobody has said so about `agent` itself.
- A narrower `agents` replaces the wider one whole, under the precedence every
  other setting uses. "Narrowed or reordered per repository" is met by a
  personal repository-scoped list (`lf set agents …`); a shared repository file
  does not outrank a personal global list.
- Status keeps reading the move from the reason's `; continuing on <agent>`
  suffix. The recorder already wrote that suffix before this Task; no person
  sees the difference a column would make.
- A launch refused on an unrouted login (`account_id: null`) has no reset time,
  so it moves through the order once and then fails with the limit.

## Open product choices

Each has a default built; none stops the work.

1. What a bare `claude` entry means when a limit refuses it. Built: with
   another entry behind it, it goes straight to that entry on its configured
   model; with none behind it (`agents: [claude]`, a lone `default_agent`, a
   pinned `agent: claude`) it tries Opus, Sonnet, Haiku. So the same entry
   behaves differently by list length, and the old `default_agent:
   claude,codex` no longer tries Sonnet and Haiku before Codex. The two
   one-rule alternatives: a bare entry always tries its harness's models
   before the next entry (what shipped before; `claude:opus` opts out), or the
   built-in ladder goes and only written entries are tried. It matters because
   it decides whether `[claude, codex]` prefers Haiku or Codex after Opus.
2. `supported_harnesses` is a second list of harnesses, read only by the model
   picker. Should the picker read `agents` and the setting go?
3. "model unavailable" was named beside usage limits as a reason to move. Only
   usage limits move a step.
4. Does a wider `default_agent` lead a narrower `agents`? Said so far: "Only
   for the scope at which the default_agent works" and "agents: [a,b,c] /
   default_agent: d / then the real order is d,a,b,c", both about one level.
   Nobody has said what a global `default_agent: claude` does to
   `skill.review.agents: [codex]`. Default to build: the narrower list is the
   whole order, because the Task says "narrowed". The alternative keeps
   today's run (Claude, then Codex) and makes `agent` the only way to get
   "Codex only"; then the docs' two "only" comments go. It matters because it
   decides whether `agents` can take a provider away from a skill or Wave, or
   only add ones behind the default.

## Checks

- `cargo test -p loopflow --lib -- config:: agent::tests:: lf::commands::config engine::config`: 143 pass (after compress).
- Compress: `_split_agent_list` now keys off `old_default_agent_warning`; a
  `default_agent: ','` with no entries is left as written instead of dropped.
- `cargo test -p loopflow --test integration documented_commands::`: pass.
- `cargo clippy --all-targets -- -D warnings`: clean.
- Gate: `(test(config::) | test(agent::tests::) | test(ops::task_execution::) | test(settings_launch_tests::) | test(documented_commands::) | test(configuration_changes_reach_an_existing_subscription) | test(graph_defaults_follow_task_wave_and_repository) | test(wave_setup_tests::)) | binary(two_people)`.
- Not run: a real Graph step refused by a provider limit. Demo: set
  `agents`, run a step on an exhausted Codex login, read `lf task status`.
