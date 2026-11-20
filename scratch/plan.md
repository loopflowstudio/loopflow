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
`/usr/local/bin`, `/usr/local/sbin`, `~/.cargo/bin` (qa added `~/.opencode/bin`), with
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

The pane's `lf session resume` is a transport viewer
(`rust/loopflow-execution/src/transport.rs`). It starts the launcher in a held
terminal through the Machine server (`start_held`, which passes the viewer's
environment as an explicit map) and writes `attached` to the receipt when the
launcher reports the provider process started (`Event::Return` →
`acknowledge_open`).

**The rule.** An opening has two lf processes: the viewer in the pane and the
launcher in its held terminal. Only they answer, with lf's own error, and the
first answer stands (`answer_open`: write a temporary, `hard_link` it to the
receipt, which fails when the receipt exists). Anything else leaves the shell
wrapper's "Conversation exited before attachment (status N)".

**Built (slices 1–4):** `answer_open`, `end_open` called from `main`,
first-answer-wins, `Attachment::settle` reporting the receipt text plus
"; retry the same command".

**Built from the second concept review:** `transport::initialize` takes
`LF_OPEN_RECEIPT` out of the environment into the `OPEN_RECEIPT` static. It is
put back by name in three places, each the same opening continuing:
`start_held`'s environment map (the launcher), the viewer's two re-execs before
the acknowledgement (`relay_with_preview`, and the relay that becomes the
installed lf on a revision skew), and `transport::carry_across_exec` (the entry
gate at `~/.local/bin/lf` runs `initialize` before it execs the selected
installation, so without this an installed lf would drop every receipt). The
relayed tail and the `native_session.rs` strip are gone; `provider_auth`'s
regex is as it was. If `usable` is later made to wait until the provider takes
input, a tail belongs with that change.

**Who answers, in order:** the launcher's own error, else the viewer's own
error, else the wrapper's status line.

**Absent/error states:** receipt directory gone (caller timed out and dropped
it) → the write fails silently.

**Tests:** `the_first_answer_to_an_opening_stands` (kept, driving the static);
a unit test that after the take the process environment holds no
`LF_OPEN_RECEIPT`; `a_failed_opening_reports_the_conversations_own_reason`
(kept); the fixture `a_launch_that_found_no_provider_leaves_nothing_to_resume`
reads the launcher's exact error from the receipt, which shows the hand-off
through `start_held` works. It returns early, printing why, on a Machine with
`claude` in a system tool directory.

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
   its conversation to the end can't answer for its viewer. The variable is no longer
   ambient and the tail is gone (section 2).
3. **No recorded `--agent`** — done. `Resolved.agent` deleted; `resume_argv`
   reads `agent_override()`.
4. **Plists, systemd, SSH, Swift** — done. `REMOTE_PATH` is `remote_path()`.

QA ran both by hand on 2026-11-19 (`scratch/qa.html`): the `env -i` resume
started real Claude against a launchd-run private server, and `lf open --json`
with the provider missing returned its reason on tmux, Rex and Herdr. QA also
made `lf open --host tmux` work on tmux 3.7 (window lookup before `new-window`)
and added `~/.opencode/bin` to the tool directories.

## Remaining

- **Bare-`PATH` fixtures no longer hide Machine tools.** Every `lf` a test
  starts now appends the tool directories, so `PATH=/usr/bin:/bin` hides
  nothing in `/opt/homebrew/bin` or `/usr/local/bin`. Seen on this Machine
  (2026-11-19): `/opt/homebrew/bin/gh` exists; no `claude` or `codex` in either
  directory. Some twenty fixtures under `rust/loopflow/tests/` set such a
  `PATH` (`grep -rn '"PATH", *"/usr/bin:/bin' rust/loopflow/tests`). Only the
  focused tests ran after the change; the suites have not. CI's Linux runners
  never hid `gh` this way, so the exposure is a macOS Machine: a fixture that
  counted on `gh` being absent now reaches an unauthenticated real one. Gate
  reads the first CI run for this, macOS job included.
- **The entry-gate hand-off has no end-to-end check.** With the default home
  a pane's command names the gate, so every ordinary `lf open` crosses it. The
  gate is found from the OS account and `lf install` takes only published
  builds, so qa could not put this branch behind one. After the first published
  build: `lf install --dev`, then from a tmux pane `lf open --task <ID> --tab
  --host tmux --json -a pi` must answer `failed` with Pi's reason and a plain
  open `usable`. The unit test covers `carry_across_exec` until then.

- **A late launcher exit logs as a failed opening.** The launcher keeps its
  receipt for the conversation's life, so a provider that exits non-zero long
  after attaching writes "conversation opened by lf open failed before
  attaching" to `logs/lf.log` (`transport::end_open`). Log only; the receipt is
  already written or its directory gone. The base did the same. Left as found
  by the third concept review; the fix is to log only while the receipt's
  directory exists and the receipt does not.

## Larger than this change (on `scratch/concept-review.html`)

- `usable` means "the provider process was started", not "it takes input".
- A failed `lf open -a <harness>` has already moved the conversation: the move
  is recorded before the harness is known to start.
- "Not installed" is said of tools a terminal finds (Pi under nvm): lf looks in
  seven directories, a person expects what their shell finds.

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
- `cargo test -p loopflow --test transport_tests a_launch_that_found_no_provider` — passed at c8779a386c (ran in full, not the early return).
- `cargo test -p loopflow-execution --lib open_receipt_tests` — passed at c8779a386c (2 tests).
- `env -i … lf session resume <throwaway>` — passed in qa, by hand.
- The Rust suites on macOS and Linux — CI; watch fixtures with a bare `PATH`.
- `lf open --task <ID> --json` with the provider missing, on tmux, Rex (`-a pi`) and Herdr — passed in qa; tmux and Rex again at 815013de83. cmux and Desktop not run: gate or a person.

Open PRs on the same files: #1978, #1976, #1967 (`native_session.rs`,
`claude.rs`), #1972 (`server/mod.rs`). Rebase before slice 1 lands.
