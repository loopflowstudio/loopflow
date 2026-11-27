# LOO-1119: Remove the Discord bridge

While `lf discord serve <wave>` ran, any non-bot member of the bound channel
got an unattended agent with permissions skipped, and its answer was posted
back: no author allowlist, and the configured `guild_id` was never read. The
bridge is removed, not fixed.

## Outcome

`lf discord` is not a command. No lf code reads a Discord token, polls a
channel or posts to one. A Wave's `chat:` block means nothing. The prompt has
two surface contracts, headless and human-present.

## State of the checkout

Uncommitted, 30 files, +28 −392. Already done and checked by the Task's
definition (`cargo check -p loopflow --all-targets` clean; focused suites pass):

- `rust/loopflow/src/lf/commands/discord.rs`, `Commands::Discord`,
  `DiscordCommand`, the `lf.rs` dispatch arm and `run::answer_bound` are gone.
- `WaveChatConfig`, `try_read_wave_chat_config`, `WaveConfig.chat` and their
  test are gone; `chat:` is out of `.lf/wave/product/config.yaml` and `GOAL.md`.
- `DISCORD_TOKEN_ENV` and every `env_remove`/`SESSION_AUTH_ENV` use are gone.
- `session_record::final_answer` (the bridge was its only caller) is gone.
- Docs: `docs/work.md` "Talk to it from Discord", `docs/dev/machines.md`
  "Independent bridges", and the mentions in glossary, sessions, security,
  environment, codebase, reference, index, harness/transport/server/swift READMEs.

`git grep -i discord` now finds only the community invite link (`README.md`,
`design/prototypes/homepage-next`), `release/*/NOTES.md` and two Wave
`MEMORY.md` lines. Those stay: shipped records and a community link.

## What remains

### 1. A leftover `chat:` block warns on every command (found by probe)

Dropping `"chat"` from `WAVE_ONLY_KEYS`
(`rust/loopflow-execution/src/work/wave/config.rs:24`) sends a leftover
`chat:` block into the shared settings layer. Probe with this checkout's
`target/debug/lf`, a Wave `config.yaml` holding `chat:`:

```
$ lf --wave scan config settings
warning: …/.lf/wave/scan/config.yaml:2: skipping chat: unknown setting
```

111 `loopflow.*` worktrees and the main checkout carry that block in
`.lf/wave/product/config.yaml` until each takes the merge, so an installed
build would print the warning in all of them. `GOAL.md` frontmatter is
unaffected: `parse_goal_settings` already drops keys it doesn't know.

Decision (made for review): `"chat"` goes back into `WAVE_ONLY_KEYS`, read by
nothing, the same treatment `linear` has there, with a comment naming the Task
that deletes it. That Task is filed in this change (`lf task create`), per the
shared-file rule in AGENTS.md. Passed over: `RETIRED_SETTINGS` in
`config/settings.rs` (it is for every scope's personal files and LOO-919
deletes it; `chat` was only ever a Wave key), and accepting the warning (noise
in 111 checkouts for a removed feature).

Test: `read_wave_layer` on a config with a `chat:` block returns a layer
without it. It fails if the key is dropped from the list again.

### 2. The chat prompt surface

Steer: "Also remove the chat prompt surface: `Surface::Chat` in
rust/loopflow-execution/src/prompt/mod.rs (variant, its `"chat"` parse arm,
`instructions` arm and test), `SURFACE_CHAT` in
rust/loopflow-definitions/src/builtins.rs, and
rust/loopflow-definitions/src/builtins/surfaces/chat.md."

Findings:

- Nothing constructs `Surface::Chat`; the only references are the variant, the
  two match arms and `chat_surface_speaks_plainly_and_is_distinct_from_headless`
  (`prompt/mod.rs:2054`). The bridge at HEAD never emitted it either; it is a
  leftover of #1360 ("Remove the resident Wave service and chat surfaces").
- A recorded `"chat"` deserializes as `Headless` through `#[serde(other)]`, and
  `FromStr` falls to `Headless` through its `_` arm. No migration.
- `trace.rs:93/115` `Chat` is `ContextAssetKind::Chat`, a recorded context
  asset kind, a different enum. Untouched.
- Swift has no mirror of `Surface`.

Removal: the variant and its doc comment, the `"chat"` parse arm, the
`instructions` arm, the test, `SURFACE_CHAT` and `surfaces/chat.md`. The
existing `surface_parser_unknown_defaults_to_headless` covers the fallback; add
`"chat"` to it so a recorded chat surface is named.

### 3. Small leftovers in the diff

- `run::run_bound_prompt` returned the capture directory only for
  `answer_bound`; both remaining callers `.map(|_| ())`. Fold it into
  `run_bound` returning `Result<()>` if `run_prompt`'s other callers allow.
- `harness/mod.rs`: `.env("LF_HOME", …)` chain reformats under `cargo fmt`.

## Acceptance check

```bash
target/debug/lf discord serve product     # → clap: unrecognized subcommand, exit 2
target/debug/lf help | grep -ci discord   # → 0
git grep -n -i 'discord' -- rust swift docs python   # → nothing
git grep -n 'Surface::Chat\|SURFACE_CHAT' -- rust    # → nothing
# a Wave config.yaml with a chat: block → `lf --wave <w> config settings` prints no warning
cargo test -p loopflow-execution --lib work::wave::config:: prompt::
cargo test -p loopflow --lib -- documented_commands wave_resolution_matrix lf::tests
```

Demo: `lf discord serve product` answers "unrecognized subcommand", and
`lf help` lists no Discord command.

## Absent and error states

- `chat:` in a Wave `config.yaml` or `GOAL.md`: ignored silently.
- Recorded Session with surface `chat`: reads as headless.
- `LF_DISCORD_TOKEN` set in the environment: lf no longer strips it from agent
  and detached launches; it passes through like any other caller variable.
  Accepted: lf has no knowledge of the name any more.

## Security and privacy

The change removes an unauthenticated remote trigger and adds no input path.
No `lf security-review` run: the Task comes out of one, and deletion is the
finding's fix. Two facts live outside the repository and go in the PR body as
post-merge steps: revoke the bot token and delete `LF_DISCORD_TOKEN` from
Doppler; remove the bot from guild `1333122108699709535`. No
`lf discord` process was running on this Machine at planning time (`pgrep`);
other Machines (mini-heart) were not checked.

## Exclusions

- `ContextAssetKind::Chat` and its recorded rows.
- Release notes and Wave `MEMORY.md` history.
- The `linear` Wave key and the `GOAL.md` frontmatter fallback (LOO-640).
- Other worktrees' `chat:` blocks: they lose it when they take the merge.

## One pass

1. `"chat"` back in `WAVE_ONLY_KEYS` with its removal Task filed, plus the test.
2. Chat surface removal.
3. `run_bound_prompt` fold, `cargo fmt`.
4. `cargo check -p loopflow --all-targets`, the focused tests above, the
   acceptance commands, `scripts/check_architecture.py`.

Then commit, review, PR, land.
