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

## Built

- §1 Leftover `chat:` block: `RETIRED_WAVE_KEYS` (`linear`, `chat`) beside a
  one-key `WAVE_ONLY_KEYS`; LOO-1121 deletes the list (736ce4cbeb). The
  `GOAL.md` fallback keeps retired keys too, so a first edit still carries
  `linear` into `config.yaml` for released lf.
- §3 `run_prompt` returns `Result<()>`; `run_bound_prompt` folded into
  `run_bound` (ff48cc001b). `harness/mod.rs` needed no reformat.
- §2 `Surface { HumanPresent, Headless }`, serde derives gone, `SURFACE_CHAT`
  and `surfaces/chat.md` deleted (050966bd30). One thing deserialized the
  prompt's `Surface`: the golden prompt cases (`tests/goldens/*.yaml`, all
  `headless`). They now hold a string parsed with `FromStr`, as `lf-prompt`
  does, so the name has one spelling.

What a run showed (2026-11-26, this checkout's `target/debug/lf`):
`lf discord serve product` exits 2 with "skill or graph not found: discord",
not Clap's "unrecognized subcommand": an unknown first word is looked up as a
skill. A Wave `config.yaml` with `agent: claude` and a `chat:` block prints
no warning from `lf --wave scan config settings` and still shows the agent.
Passed: execution `work::wave::config::` (7), the surface and prompt tests
(8), `golden_prompt`, `context_tests::gather_context_preserves_surface`,
`cargo check -p loopflow --all-targets`, `scripts/check_architecture.py`.
Left to CI: clippy, `lf::tests::`, `documented_commands::`, the suites.

What remains: review, PR, land.

## Acceptance check

```bash
target/debug/lf discord serve product     # → skill or graph not found: discord, exit 2
target/debug/lf help | grep -ci discord   # → 0
git grep -n -i 'discord' -- rust swift docs python   # → nothing
git grep -n 'Surface::\(Chat\|Cli\|Ide\|Mac\|Iphone\)\|SURFACE_CHAT' -- rust   # → nothing
# a Wave config.yaml with a chat: block → `lf --wave <w> config settings` prints no warning
cargo test -p loopflow-execution --lib work::wave::config:: prompt::
cargo test -p loopflow --lib -- documented_commands wave_resolution_matrix lf::tests
cargo test -p loopflow --test integration context_tests   # find the binary that holds context_tests.rs first
```

Demo: `lf discord serve product` answers "skill or graph not found: discord", and
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
