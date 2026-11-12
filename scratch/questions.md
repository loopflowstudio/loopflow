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

## Built: a narrower list is the whole order (2026-11-12)

Concept review 2 ran global `default_agent: claude` with
`lf set --global skill.implement.agents codex` and saw `agent  claude, then
codex`. Now `default_agent` leads only when its winning declaration ranks at
least as high as the winning `agents` by (source, scope, target); otherwise
the list is the whole order and `--explain` marks the `default_agent`
"unused: a more specific agents list is the whole order". The same comparison
drops the sole installed harness under any authored list.

Run in a throwaway home with that config:
`lf config settings --skill implement --explain` showed `agent  codex
Personal global · skill implement`, the unused line for `default_agent =
claude`, and `selected` for `agents = ["codex"]`; plain `lf config settings`
showed `agent  claude`. `default_agent: claude,codex` warned once (it printed
twice before; the warning is now once per file per process).

Consequence of ranking by source first: a repository `default_agent` (general
or per skill) no longer leads a personal `agents` list, since personal
outranks repository. `docs/config.md` precedence table says so. A person with
only a personal list and no `default_agent` no longer follows a repository's
fallback; nobody has said which is wanted.

## Next implement direction (concept review 3, 2026-11-12)

The rule is rank (source, then scope, then target), and the words say
"specific". Run in a throwaway home with personal global `agents: [claude,
codex]` and a repository `.lf/config.yaml` holding
`skill.implement.default_agent: codex`:
`lf config settings --skill implement --explain` showed `agent  claude, then
codex` and, under `default_agent = codex · Shared project · skill implement`,
`unused: a more specific agents list is the whole order`. The list there is
global and the fallback is per skill; the reason sends a reader looking for a
skill-level list that does not exist.

1. The reason reads `unused: a higher agents list is the whole order`, the
   word the neighbouring `overridden by a higher declaration` uses.
2. `docs/config.md` says the same in both places: in "Keep going when a
   provider runs out" ("A wider `default_agent` does not lead a narrower
   list … at least as specifically") and in the precedence paragraph
   ("declared more specifically"). State it by rank, and give the repository
   case one sentence beside the skill one: a repository `default_agent` does
   not lead a personal list.
3. Comments and the test in `config/settings.rs` follow the wording.

Done looks like: that repro prints the new reason; the focused config tests
pass. No behaviour changes.

Also run, unchanged and as designed: the incident's config (global
`default_agent: claude`, `skill.compress.default_agent: codex`, no `agents`)
still shows compress as `codex` alone; after `lf set --global agents
claude,codex` it shows `codex, then claude`, and everything else `claude,
then codex`.

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
   Built: the narrower list is the whole order, because the Task says
   "narrowed". The alternative keeps Claude, then Codex and makes `agent` the
   only way to get "Codex only". Undecided inside the built rule: whether a
   repository `default_agent` should lead a personal list (built: it does not).

## Checks

- `cargo test -p loopflow --lib -- config:: agent::tests:: lf::commands::config engine::config`: 143 pass (after the narrowing rule).
- Compress: `_split_agent_list` now keys off `old_default_agent_warning`; a
  `default_agent: ','` with no entries is left as written instead of dropped.
- Compress 2: dropped a narrowing test case that could not fail (global
  `default_agent: claude` over a Wave `agents: [claude]`); focused tests 143
  pass, clippy clean.
- `cargo test -p loopflow --test integration documented_commands::`: pass.
- `cargo clippy --all-targets -- -D warnings`: clean.
- Gate: `(test(config::) | test(agent::tests::) | test(ops::task_execution::) | test(settings_launch_tests::) | test(documented_commands::) | test(configuration_changes_reach_an_existing_subscription) | test(graph_defaults_follow_task_wave_and_repository) | test(wave_setup_tests::)) | binary(two_people)`.
- Not run: a real Graph step refused by a provider limit. Demo: set
  `agents`, run a step on an exhausted Codex login, read `lf task status`.
