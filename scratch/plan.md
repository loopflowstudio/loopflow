# LOO-1119: Remove the Discord bridge

While `lf discord serve <wave>` ran, any non-bot member of the bound channel
got an unattended agent with permissions skipped, and its answer was posted
back: no author allowlist, and the configured `guild_id` was never read. The
bridge is removed, not fixed.

## Outcome

`lf discord` is not a command. No lf code reads a Discord token, polls a
channel or posts to one. A Wave's `chat:` block means nothing. The prompt is
told whether someone is in the conversation by the launch's run mode
(`harness::Mode`); there is no second type for it.

## What exists (committed at 2365b1f548)

- `rust/loopflow/src/lf/commands/discord.rs`, `Commands::Discord`,
  `DiscordCommand`, the `lf.rs` dispatch arm and `run::answer_bound` are gone.
- `WaveChatConfig`, `try_read_wave_chat_config`, `WaveConfig.chat` and their
  test are gone; `chat:` is out of `.lf/wave/product/config.yaml` and `GOAL.md`.
- `DISCORD_TOKEN_ENV` and every `env_remove`/`SESSION_AUTH_ENV` use are gone.
- `session_record::final_answer` (the bridge was its only caller) is gone.
- Docs: `docs/work.md` "Talk to it from Discord", `docs/dev/machines.md`
  "Independent bridges", and the mentions in glossary, sessions, security,
  environment, codebase, reference, index, harness/transport/server READMEs.
- `swift/README.md`: the Discord sentence and the Wave Chat sentences about
  backings, backing epochs and SSE. No Swift source has any of it.
- `docs/dev/reference.md` no longer lists chat among
  what a Wave holds, and its vocabulary appendix calls `project:<slug>` a work
  binding, not an "authored chat reference". No other shipped doc needs an
  edit: nothing documents `lf-prompt --surface` outside its Clap help (§2),
  and a leftover `chat:` block is silent, so no doc mentions it.

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

- The prompt takes the launch's `harness::Mode`; `prompt::Surface` is deleted
  (b95737849e). Made for review (concept review question 1a). Nothing needed the
  prompt's value to differ from the launch's mode. Fields are named `mode`;
  `Mode` gained `Default` (headless), `clap::ValueEnum` and `instructions()`.
  The instruction text follows the type: `MODE_HEADLESS`, `MODE_INTERACTIVE`
  and `builtins/modes/{headless,interactive}.md` (file contents unchanged, so
  the rendered prompt is the same bytes). Kept under the old word: the trace
  kind `Kind::SurfaceInstructions` (`surface_instructions` is in a SQL `CHECK`
  since the first migration) and the recorded Session `surface` string
  (deferred, below). Attribution labels read "Headless mode" / "Interactive
  mode".

What a run showed (2026-11-26): `lf-prompt --mode cli` exits 2 with "invalid
value 'cli' for '--mode <MODE>' [possible values: interactive, headless]";
`--surface headless` is "unexpected argument". `golden_prompt` passed with
the rendered files untouched; `context_tests::` 16 passed; execution
`prompt::` 101 passed, 1 failed:
`prompt::process::tests::builtin_skills_use_the_application_fallback` got
"opencode" where it expects "codex". It passes `Mode::Headless` as it passed
`Surface::Headless`; the agent comes from `MachineAgents::choose`, which
probes this Machine's installed harnesses and logins before the mode is
read. Not run at the parent commit. Left to CI.

## Remaining

Review, PR, land. Deferred: typing the recorded Session `surface` string
(`run.rs` `surface: &str`, `SessionCaptureSpec.surface`).

## Acceptance check

```bash
target/debug/lf discord serve product     # → skill or graph not found: discord, exit 2
target/debug/lf help | grep -ci discord   # → 0
git grep -n -i 'discord' -- rust swift docs python   # → nothing
git grep -n -E 'prompt::Surface|\bSURFACE_|builtins/surfaces' -- rust   # → nothing
target/debug/lf-prompt --mode cli   # → Clap error naming headless, interactive
# a Wave config.yaml with a chat: block → `lf --wave <w> config settings` prints no warning
cargo test -p loopflow-execution --lib work::wave::config:: prompt::
cargo test -p loopflow --lib -- documented_commands wave_resolution_matrix lf::tests
cargo test -p loopflow --test integration context_tests   # find the binary that holds context_tests.rs first
```

Demo: `lf discord serve product` answers "skill or graph not found: discord", and
`lf help` lists no Discord command.

## Absent and error states

- `chat:` in a Wave `config.yaml` or `GOAL.md`: ignored silently.
- `lf-prompt --mode <anything but headless or interactive>`: a Clap error.
  `--surface` is not a flag.
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

Each finding of `scratch/concept-review.html` at f342908097 (it replaces the
page at 200d4e486a, whose findings the Mode pass built):

- The mode's instruction text was still named for a surface (`SURFACE_*`,
  `builtins/surfaces/human-present.md`) → renamed here to the mode's words.
  `cargo check -p loopflow-execution` passed; `golden_prompt` not rerun, the
  two files' contents did not change.
- The recorded Session keeps its own copy as a word (`"tui"`, `"headless"`,
  and `interactive: bool` derived from it) → separate outcome, deferred below.
- Token still stored in Doppler `loopflow/dev` and no longer stripped → the
  open question above, unchanged. An old installed `lf` can still run the
  bridge until it is revoked.
- Who asked for this agent → deferred below, unchanged.
- Everything else on the page matches and is built.

## Deferred findings

- **Who asked for this agent.** lf has no type for the author of a launch
  request, and needs none while every launch comes from the Machine's own
  account. The first inbound trigger (a webhook, a self-hosted server) needs
  a named author on the request and a rule for which authors may start an
  agent, designed before that trigger is built. Nothing to do in this Task.
- **Type the recorded Session `surface` string.** `run.rs` `run_prompt`
  writes `"tui"` or `"headless"` from the same `Mode`. A `Mode`-typed field on
  `SessionCaptureSpec` that writes the same two words under the same key: no
  migration. Two readers disagree and both must keep working:
  `session_record/capture.rs` treats any word but `"headless"` as interactive,
  `commands/replay.rs` requires exactly `"headless"`. About 27 hand-spelled
  sites, mostly harness test fixtures; one (`transport.rs`, the resume
  delegation table) writes `"interactive"`, which no launch writes. No Task
  is filed for it.

## Exclusions

- `ContextAssetKind::Chat` and its recorded rows.
- Release notes and Wave `MEMORY.md` history.
- The `GOAL.md` frontmatter fallback (LOO-640).
- Other worktrees' `chat:` blocks: they lose it when they take the merge.
