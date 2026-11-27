# LOO-1119: Remove the Discord bridge

While `lf discord serve <wave>` ran, any non-bot member of the bound channel
got an unattended agent with permissions skipped, and its answer was posted
back: no author allowlist, and the configured `guild_id` was never read. The
bridge is removed, not fixed.

## Outcome

`lf discord` is not a command. No lf code reads a Discord token, polls a
channel or posts to one. A Wave's `chat:` block means nothing. The prompt has
two surface contracts, headless and human-present, and the type has two
variants.

## What exists (committed at e577962da2)

- `rust/loopflow/src/lf/commands/discord.rs`, `Commands::Discord`,
  `DiscordCommand`, the `lf.rs` dispatch arm and `run::answer_bound` are gone.
- `WaveChatConfig`, `try_read_wave_chat_config`, `WaveConfig.chat` and their
  test are gone; `chat:` is out of `.lf/wave/product/config.yaml` and `GOAL.md`.
- `DISCORD_TOKEN_ENV` and every `env_remove`/`SESSION_AUTH_ENV` use are gone.
- `session_record::final_answer` (the bridge was its only caller) is gone.
- Docs: `docs/work.md` "Talk to it from Discord", `docs/dev/machines.md`
  "Independent bridges", and the mentions in glossary, sessions, security,
  environment, codebase, reference, index, harness/transport/server READMEs.
- `swift/README.md`: the Discord sentence, and (uncommitted, from the concept
  review) the Wave Chat sentences about backings, backing epochs and SSE. No
  Swift source has any of it.

- Docs pass (uncommitted): `docs/dev/reference.md` no longer lists chat among
  what a Wave holds, and its vocabulary appendix calls `project:<slug>` a work
  binding, not an "authored chat reference". No other shipped doc needs an
  edit: nothing documents `lf-prompt --surface` outside its Clap help (§2),
  and a leftover `chat:` block is silent, so no doc mentions it.

Checked at kickoff: `cargo check -p loopflow --all-targets` clean; the focused
suites under Acceptance passed. Nothing has been rebuilt since.

`git grep -i discord` finds only the community invite link (`README.md`,
`design/prototypes/homepage-next`), `release/*/NOTES.md` and two Wave
`MEMORY.md` lines. Those stay: shipped records and a community link.

## What remains: one pass

### 1. A leftover `chat:` block warns on every command

Dropping `"chat"` from `WAVE_ONLY_KEYS`
(`rust/loopflow-execution/src/work/wave/config.rs:24`) sends a leftover
`chat:` block into the shared settings layer. Probe at kickoff with this
checkout's `target/debug/lf`, a Wave `config.yaml` holding `chat:`:

```
$ lf --wave scan config settings
warning: …/.lf/wave/scan/config.yaml:2: skipping chat: unknown setting
```

111 `loopflow.*` worktrees and the main checkout carry that block in
`.lf/wave/product/config.yaml` until each takes the merge. `GOAL.md`
frontmatter is unaffected: `parse_goal_settings` drops keys it doesn't know.

A leftover `chat:` block is ignored silently, and a removal Task is filed in
this change (`lf task create`), per the shared-file rule in AGENTS.md.

Made for review (concept review, question 3a): the list splits in two. `id`
is the one key a Wave owns; `linear` and `chat` go in a list of retired Wave
keys that nothing reads, each with the Task that deletes it (`chat`: the Task
filed here; `linear`: find its Task before naming one, LOO-640 is the
`GOAL.md` fallback and may not cover it). Passed over: `chat` beside `linear`
in the one existing list with a comment (smaller, but the list's name then
says "keys a Wave carries" for two keys nothing reads); `RETIRED_SETTINGS` in
`config/settings.rs` (every scope's personal files, and LOO-919 deletes it);
letting it warn.

Test: `read_wave_layer` on a config with a `chat:` block returns a layer
without it. It fails if the key is dropped again.

### 2. `Surface` becomes two variants

Steer: "Also remove the chat prompt surface: `Surface::Chat` in
rust/loopflow-execution/src/prompt/mod.rs (variant, its `"chat"` parse arm,
`instructions` arm and test), `SURFACE_CHAT` in
rust/loopflow-definitions/src/builtins.rs, and
rust/loopflow-definitions/src/builtins/surfaces/chat.md."

That removal stands. Made for review (concept review, question 1a): the same
edit takes `Cli`, `Ide`, `Mac` and `Iphone` down to one `HumanPresent`, so the
enum has one variant per instruction file:

```rust
pub enum Surface { HumanPresent, #[default] Headless }
```

Why here: the only launcher choice is `run.rs:518` (`Cli` when interactive,
else `Headless`); `Ide`, `Mac`, `Iphone` appear only in tests
(`prompt/mod.rs:1880`, `prompt/process.rs:527`) and share `Cli`'s text.
Passed over: removing only `Chat` and filing the rest as its own Task (the
steer's literal scope; four names for one answer stay), and keeping `Mac` and
`Iphone` as placeholders for the apps. If the iOS app needs its own
instructions, that is a third file and a third variant then.

What the pass does:

- `Surface`: two variants; `instructions` matches both by name; `FromStr`
  takes `human-present` and falls to `Headless` for anything else.
- Drop the `Serialize`/`Deserialize` derives and `#[serde(...)]` attributes
  and build. The prompt's `Surface` is never stored: `PromptComponents` and
  `GatherContextOpts` have no serde, and a recorded Session's `surface` is a
  different thing, a `String` holding `"tui"` or `"headless"`. If the build
  fails, something reads one: keep the derive and write down what.
- `lf-prompt --surface` (`rust/loopflow/src/bin/lf-prompt.rs:24`): help text
  says `headless/human-present`. Its two callers
  (`tests/e2e/test_smoke.sh:42`, `scripts/evals/instruction_levels.py:53`)
  pass `headless`. `cli`, `mac`, `iphone`, `ide` now read as headless; no
  aliases.
- Delete `SURFACE_CHAT`, `surfaces/chat.md`,
  `chat_surface_speaks_plainly_and_is_distinct_from_headless`, and the parser
  test that expects `Ide`. `surface_parser_unknown_defaults_to_headless`
  stays as it is.
- Rename `Surface::Cli` at its uses: `run.rs:519`, `prompt/process.rs`,
  `prompt/mod.rs` tests, `rust/loopflow/tests/context_tests.rs`. The two
  loops over human surfaces become one case.
- `trace.rs` `ContextAssetKind::Chat` is a recorded context asset kind, a
  different enum. Untouched.

### 3. Small leftovers

- `run::run_bound_prompt` returned the capture directory only for
  `answer_bound`; both remaining callers `.map(|_| ())`. Fold it into
  `run_bound` returning `Result<()>` if `run_prompt`'s other callers allow.
- `harness/mod.rs`: `.env("LF_HOME", …)` chain reformats under `cargo fmt`.

### 4. Check

`cargo check -p loopflow --all-targets`, the focused tests and acceptance
commands below, `scripts/check_architecture.py`. Then commit, review, PR, land.

What the pass teaches: whether anything deserializes the prompt's `Surface`
(the derive drop answers it), and whether `run_prompt`'s callers let the
capture directory go.

## Acceptance check

```bash
target/debug/lf discord serve product     # → clap: unrecognized subcommand, exit 2
target/debug/lf help | grep -ci discord   # → 0
git grep -n -i 'discord' -- rust swift docs python   # → nothing
git grep -n 'Surface::\(Chat\|Cli\|Ide\|Mac\|Iphone\)\|SURFACE_CHAT' -- rust   # → nothing
# a Wave config.yaml with a chat: block → `lf --wave <w> config settings` prints no warning
cargo test -p loopflow-execution --lib work::wave::config:: prompt::
cargo test -p loopflow --lib -- documented_commands wave_resolution_matrix lf::tests
cargo test -p loopflow --test integration context_tests   # find the binary that holds context_tests.rs first
```

Demo: `lf discord serve product` answers "unrecognized subcommand", and
`lf help` lists no Discord command.

## Absent and error states

- `chat:` in a Wave `config.yaml` or `GOAL.md`: ignored silently.
- `lf-prompt --surface <anything but human-present>`: headless.
- `LF_DISCORD_TOKEN` set in the environment: lf no longer strips it from agent
  and detached launches; it passes through like any other caller variable.

## Security and privacy

The change removes an unauthenticated remote trigger and adds no input path.
No `lf security-review` run: the Task comes out of one, and deletion is the
finding's fix.

Post-merge steps outside the repository, for the PR body: revoke the bot
token and delete the Discord secret from Doppler; remove the bot from guild
`1333122108699709535`.

The concept review asked for the revocation before the merge (question 2a):
once lf forgets the name, `doppler run -- lf …` hands a working token to the
agent. Open, in `scratch/questions.md`; revoking cannot be undone and is a
person's to do. Made for review: the merge does not wait for it, and lf keeps
no Discord name to strip. What a run showed (2026-11-26): the Doppler project
`loopflow` has a Discord-named secret in `dev` only; `prd` (this checkout's
default) and `stg` have none. `SESSION_AUTH_ENV` strips two names, `GH_TOKEN`
and `OPENCODE_API_KEY`, so an agent started under `doppler run -c dev`
already receives every other secret in that config; the token joins them
until it is revoked.

No bridge is running: `pgrep -fl 'lf discord'` found none on this Machine at
kickoff, and none here or on 100.96.227.95 (the cron host) during the concept
review. Other Machines were not checked.

## Concept review, answered

Each finding of `scratch/concept-review.html` at e577962da2:

- Six surface names for two answers → adopted, §2, made for review.
- "surface" names two ideas, and the plan's "recorded Session with surface
  `chat` reads as headless" cannot occur → the absent state, the "no
  migration" finding and the `"chat"` parser-test addition are cut; the serde
  derive drop is in §2.
- "chat" used five ways → two go in §1 and §2; the Wave Chat sentences in
  `swift/README.md` are deleted here; `ContextAssetKind::Chat` stays.
- Wave key list holds keys nothing reads → §1, made for review.
- `run_bound_prompt` returns a directory nobody uses → §3, as planned.
- Token revoked after the merge instead of before → open question, above.
- mini-heart not checked → checked, above.

## Deferred findings

- **Who asked for this agent.** lf has no type for the author of a launch
  request, and needs none while every launch comes from the Machine's own
  account. The first inbound trigger (a webhook, a self-hosted server) needs
  a named author on the request and a rule for which authors may start an
  agent, designed before that trigger is built. Nothing to do in this Task.

## Exclusions

- `ContextAssetKind::Chat` and its recorded rows.
- Release notes and Wave `MEMORY.md` history.
- The `GOAL.md` frontmatter fallback (LOO-640).
- Other worktrees' `chat:` blocks: they lose it when they take the merge.
