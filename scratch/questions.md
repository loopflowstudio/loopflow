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

## Next implement (concept review, 2026-11-12)

1. `agents` reads a comma string as the list, in files and in `lf set`
   (`agents: claude,codex`, `lf set --global agents claude,codex`). The request
   spelled it that way: "we should just be doing agents: a,b". Run on this
   branch's build, in a throwaway home:
   - file `agents: claude,codex` printed `warning: …/config.yaml:1: skipping
     agents: invalid value` and then `agent    Not selected`; the warning does
     not name the list form.
   - `lf set --global agents claude,codex` printed `Error: invalid graph:
     agents needs a list, such as [claude, codex]`.
   Docs then lead with the short command and keep `[claude, codex]` for YAML.
   `config::agent_list` stays for this; LOO-875's notes list it for deletion.
2. `lf set --global default_agent claude,codex` printed `default_agent saved`
   with no warning, and the next read warned. Print the old-spelling warning
   when `lf set` saves it.
3. `docs/config.md` says "Each entry is tried as written; nothing the list
   lacks is tried." A one-entry order breaks that: `agents: [claude]` leaves
   `fallback_agents` empty, so `_fallthrough_rungs` adds Opus, Sonnet and
   Haiku. Say so there (as `docs/subscriptions.md` already does), and cover
   `agents: [claude]` in `an_order_is_tried_as_written…` or beside it. Which
   rule is right is open choice 1 below; the docs describe what is built.

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

## Checks

- `cargo test -p loopflow --lib -- config:: agent::tests:: lf::commands::config engine::config`: pass, except `config::files::tests::relocating_wave_preferences_preserves_conflicts_and_concurrent_edits` failed once in the parallel run and passes alone.
- `cargo test -p loopflow --test integration documented_commands::`: pass.
- `cargo test -p loopflow --test integration settings_launch_tests::`: pass.
- `cargo clippy --all-targets -- -D warnings`: clean.
- Compress: `cargo test -p loopflow --lib -- agent::tests:: config:: lf::commands::config`: 140 pass; clippy clean.
- Gate: `(test(config::) | test(agent::tests::) | test(ops::task_execution::) | test(settings_launch_tests::) | test(documented_commands::) | test(configuration_changes_reach_an_existing_subscription) | test(graph_defaults_follow_task_wave_and_repository) | test(wave_setup_tests::)) | binary(two_people)`.
- Not run: a real Graph step refused by a provider limit. Demo: set
  `agents`, run a step on an exhausted Codex login, read `lf task status`.
