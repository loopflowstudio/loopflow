# LOO-986: a pane's conversation starts from anywhere, says why it failed, and stays on its harness

One change to the command `lf open` puts in a host pane
(`rust/loopflow/src/lf/commands/open/conversation.rs` `Attachment::argv`) and
what that command runs. Three outcomes, built in this order.

## Done when

- `env -i HOME=$HOME TERM=xterm-256color PATH=/usr/bin:/bin:/usr/sbin:/sbin "$(command -v lf)" session resume <throwaway>`
  starts the provider. (`lf` is named by absolute path: launchd's `PATH` can't find
  `lf` either, and every real entry point names it absolutely.)
- `lf open --task <ID> --json` into Rex, tmux, Herdr or cmux with the provider CLI
  missing returns `status: failed` and a `reason` containing "'claude' is not
  installed" (or Codex's "codex CLI not found"). No `rex capture`. The pane still
  shows the error and waits for Enter.
- `lf open` on an existing conversation with no `-a` writes a pane command with no
  `--agent`; rerunning it after `lf resume -a <other>` leaves the conversation on
  the other harness. `lf open -a <harness>` still moves it.

## 1. lf finds its tools on its own

**Mechanism (made for review).** `lf`'s `main` repairs its own `PATH` before
anything else runs (first lines of `rust/loopflow/src/bin/lf.rs` `main`, before
`transport::initialize`, while the process has one thread): every tool directory
missing from `PATH` is **appended**. Everything lf starts — the holder, a
provider, `gh`, `git` — inherits it. One place covers Desktop, the supervised
server, a host pane, a launchd job and an SSH session alike, and the next entry
point needs no repair.

Passed over:
- *Add the directories where lf builds a provider's `Command`* (the brief's
  idea). Covers providers but not `gh`, `git-lfs` or the holder, so the server
  would still need its own repair; two places instead of one.
- *Prepend* (what `supervised_path` and Swift do now). Prepending reorders a
  person's chosen `PATH` when it lacks one of the directories, and lets
  `~/.local/bin` shadow what they picked. Appending only ever resolves what the
  given `PATH` could not, so a terminal's `PATH` is never changed in effect. A
  launchd `PATH` holds no `claude`, `codex`, `gh` or `node`, so appending finds
  them.

**The one list.** `loopflow_execution::os_process::TOOL_DIRS` (name for
implementer): `~/.local/bin`, `/opt/homebrew/bin`, `/opt/homebrew/sbin`,
`/usr/local/bin`, `/usr/local/sbin`, `~/.cargo/bin`, with
`fn tool_path(path: &str, home: &Path) -> String` beside it. Everything below
derives from it.

**What goes:**
- `server::supervised_path`, `SUPERVISED_ENV`/`LF_SUPERVISED` and its branch in
  `server::run` (`rust/loopflow/src/server/mod.rs`); its test moves to `tool_path`.
- `LF_SUPERVISED` in the launchd plist and systemd unit
  (`rust/loopflow-execution/src/installation/server.rs:108,214`) and
  `swift/LaunchAgents/com.loopflow.server.plist`.
- `conversation_path`, the `PATH=` push in `Attachment::argv`, and the test
  `conversation_resolves_commands_on_the_callers_path_not_the_hosts`. Replaced by
  a test that the pane command names no `PATH`, so an agent caller's shims and
  venv stay out of the conversation.
- `world::DESKTOP_PATH`: `WorldEnv::new` builds from `TOOL_DIRS`.
- Hard-coded tool directories in `install/published.rs` plists (`cleanup_plist`
  line 764 drops `/opt/homebrew/bin:/usr/local/bin`; the update plist at 977
  keeps `{bin}` first and drops the rest). These run `lf` by absolute path.
- `machine/ssh.rs REMOTE_PATH` becomes a function built from `TOOL_DIRS` (with
  `$HOME/` for `~/`). It must stay a shell snippet: the remote shell has to find
  `lf` before lf can repair anything. Its `npm config get prefix` clause stays.

**Stays:** Swift's `GUIProcessEnvironment.enrichedPath`. `LoopflowApp.swift:7`
enriches the app's own environment for what Swift spawns (`git`, the bundled
`lf`); its comment says that is its only job and names `TOOL_DIRS` as the list lf
uses for itself. `swift/project.yml` build-script `PATH`s are Xcode build steps,
not lf.

**Check:** a unit test on `tool_path` (missing appended once, present entries
keep their order, `~/` expanded); the `env -i` acceptance command against a
throwaway Session. Fixtures that hide a provider with a bare `PATH`
(`transport_tests::a_launch_that_found_no_provider_leaves_nothing_to_resume`)
now also get the tool directories; they stay hidden only because the support
ctor points `HOME` at temp. A Machine with `claude` in `/opt/homebrew/bin` or
`/usr/local/bin` would find it there — such fixtures must hide the provider
another way (a stand-in that exits "not installed", or `HOME`-relative only).

## 2. The receipt says why

Today the pane's `lf session resume` is a transport viewer
(`rust/loopflow-execution/src/transport.rs`). On success it writes `attached` to
`$LF_OPEN_RECEIPT` (`acknowledge_open`). On failure nothing in lf writes the
receipt; the shell wrapper writes "Conversation exited before attachment
(status N)" and `Attachment::settle` adds "see the destination for diagnostics".

Both messages in the acceptance are lf's own errors (`agent/mod.rs:1401`
"'claude' is not installed", `provider_auth/mod.rs:193` "{provider} CLI not
found"), but they can come from either of two processes: the pane's lf itself
(resolution failed before any view), or the launcher inside the held terminal,
whose text reaches the viewer only as relayed output followed by an exit code
(`transport.rs` ~1890 returns `CommandExit(child_code)`).

**The rule (concept review).** The receipt is written once: the first answer
stands. Every writer — the viewer's `acknowledge_open`, `fail_open` in any lf
process, the shell wrapper — creates the file only if it is absent (write the
temporary, then `std::fs::hard_link` it to the receipt, which fails when the
receipt exists, and remove the temporary; the wrapper already checks
`[ ! -f ]`). Without this the plan below has two lf processes holding
`LF_OPEN_RECEIPT` — the launcher in the held terminal inherits it from the
viewer (`native_session.rs:571` strips it only from the provider) — and the
viewer's tail written after the launcher's precise error would replace it.

**Mechanism.** One failure writer, `transport::fail_open(text)`, beside
`acknowledge_open`, under the rule above; it clears the env var, so it runs at
most once per process.
- The viewer keeps the last 4 KiB of relayed live output (`Tag::Out`, not
  `Tag::Snapshot`: replayed history of an existing conversation is not why
  this opening failed) while the receipt is unacknowledged (nothing once
  acknowledged, nothing without the env var). When
  the relay ends before acknowledgement, it calls `fail_open` with that tail,
  ANSI-stripped and trimmed, plus the exit line. This also carries a provider's
  own message when the provider itself exits before it is ready.
- `bin/lf.rs` `main`'s error path, where the `tracing::warn!` already checks the
  env var, calls `fail_open(format!("{error:#}"))` for any error that isn't a
  `CommandExit`. The warn line stays as the log record.
- The shell wrapper keeps its `[ ! -f "$receipt" ]` fallback for a command that
  died without lf writing anything (killed, `lf` missing).
- `Attachment::settle` reports the receipt text as the reason, ending "; retry
  the same command"; "see the destination for diagnostics" is dropped.

Who answers, in order: the launcher's own lf error (its `main`), else the
viewer's own error (its `main`), else the tail (a provider that exited before it
was ready, e.g. Claude's "No conversation found"), else the wrapper's status
line.

**First probe of this slice:** the bare-PATH fixture already asserts the
viewer's stdout holds "'claude' is not installed" (the held terminal relays the
launcher's stderr), so the tail alone would carry it. The probe is now
(`a_launch_that_found_no_provider_leaves_nothing_to_resume`) asserting
the receipt file is exactly the launcher's error (`'claude' is not installed. …`,
no relayed bytes around it): that shows the launcher holds the variable and
wrote first. If it holds no variable, the tail answers and the order above loses
its first rung; nothing else changes.

**Absent/error states:** receipt directory gone (caller timed out and dropped
it) → write fails silently, as `acknowledge_open` does. Empty tail → the exit
line alone ("Conversation exited before attachment (status N)").

**Test:** `Attachment` unit test running a command that prints an error and
exits 1 through the real wrapper: `wait` returns that error text. A
`transport` unit test: `fail_open("first")` then `fail_open("second")` from a
fresh env var leaves "first"; `acknowledge_open` after a failure leaves the
failure. The fixture
assertion above covers the lf side end to end.

## 3. The pane command never names a harness it wasn't given

`conversation::resolve` sets `target.agent` from the Session's recorded
provider/model, and `Resolved::resume_argv` (`open.rs:192`) writes it as
`--agent`. Replaying that command later is a move.

**Mechanism.** `target.agent = loopflow_execution::config::agent_override()` —
the invocation's `-a` (or `LF_AGENT_OVERRIDE`), nothing recorded. With none,
`lf session resume <id>` continues on the recorded provider and model
(`config::resume_choice(provider, model, None)` → `Resume::Native(model)`), and a
conversation with no provider yet resolves the harness in the pane from the same
repository and Wave settings the opener read. The `prepare && provider.is_none()`
config check stays: it surfaces a bad `agent:` before a pane opens.

If `target.agent` has no other reader after this, drop the field and read
`agent_override()` in `resume_argv`.

This is also what makes part 2's "retry the same command" safe to say: today
retrying a failed pane command can move the conversation.

**Test:** an `open.rs` unit test: a `Resolved` with a recorded provider and no
override yields argv without `--agent`; with an override, `--agent <it>`. The
`--here` path uses the same `resume_argv`, so it is covered.

## Slices

All four built (commits 54c70a2345 and the one after).

1. **PATH in main** — done. `os_process::{TOOL_DIRS, tool_dirs, tool_path,
   repair_path}`; `main` calls `repair_path()` first. Server, pane and World
   copies deleted. The bare-PATH fixture still passes: its `HOME` is temp and
   this Machine has no provider in `/opt/homebrew/bin` or `/usr/local/bin`.
2. **Receipt reason** — done. Probe answered yes: the launcher holds
   `LF_OPEN_RECEIPT` and writes first; the fixture asserts the receipt starts
   with "'claude' is not installed" and holds no exit line. `transport::end_open`
   runs in `main` for every failed command holding the variable; a clean exit
   (code 0) writes nothing and leaves the wrapper's line, so a launcher that ran
   its conversation to the end can't answer for its viewer. The CSI regex in
   `provider_auth` widened to ECMA-48 (`\x1b[?25h` leaked through before).
3. **No recorded `--agent`** — done. `Resolved.agent` deleted; `resume_argv`
   reads `agent_override()`.
4. **Plists, systemd, SSH, Swift** — done. `REMOTE_PATH` is `remote_path()`.

Not run here: the `env -i … lf session resume <throwaway>` acceptance (needs a
PTY, a server and a real provider; a branch build against the real store) and
Rex `lf open --json` with `claude` off `PATH`. Both go to qa.

## Exclusions

- **Known cost of part 1.** Today (#1981) a person running `lf open` from a
  terminal carries their whole `PATH` into a Rex pane. After this change a Rex
  pane started under launchd sees only launchd's `PATH` plus `TOOL_DIRS`, so a
  tool reachable only through shell rc (nvm, mise, asdf, pyenv shims) is gone
  from the conversation. On this Machine `node` is under `~/.nvm` but also in
  `/opt/homebrew/bin`; `claude` and `codex` are in `~/.local/bin`. The brief
  asks for `conversation_path` deleted; kept as asked, recorded in
  `scratch/questions.md`.

- Other terminal-inherited variables (`SSH_AUTH_SOCK`, locale, `TERM`) were not
  compared between a Rex pane and a shell; Herdr and cmux servers' environments
  were not checked. Not in this change.
- LOO-786 (whether `lf open --task` moves an attached conversation on every host)
  and LOO-909 (repository conversations in Rex).
- npm-prefix providers outside the tool directories under launchd: only the SSH
  snippet asks npm.

## Docs (written first)

- `docs/workspaces.md` "If opening fails": the `--json` failure with its reason,
  harness kept without `-a`, where a pane finds tools and the nvm/mise cost.
- `docs/sessions.md` "Switch agents": `lf open` keeps the switched harness.

## Gate checks

- `cargo test -p loopflow --lib open::` and `server::` — implementer.
- `cargo test -p loopflow --test transport_tests a_launch_that_found_no_provider` — implementer.
- `env -i … lf session resume <throwaway>` — implementer, by hand.
- Rex `lf open --task <ID> --json` with `claude` off `PATH` — qa.

Open PRs on the same files: #1978, #1976, #1967 (`native_session.rs`,
`claude.rs`), #1972 (`server/mod.rs`). Rebase before slice 1 lands.
