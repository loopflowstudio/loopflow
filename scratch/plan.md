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

**Changes from the second concept review (next pass).**

1. *The receipt is handed on by name, not inherited.* As built,
   `LF_OPEN_RECEIPT` is ambient: every `lf` the viewer or launcher starts
   inherits it, and `end_open` lets any of them answer. Counterexample:
   `agent_process::require_server` runs `lf install server` (`readiness_command`)
   and tolerates its failure when the server answers afterwards; that child's
   error becomes the first answer, and `lf open` reports `failed` for a
   conversation that attached. When the launch does fail, the reason is the
   install child's error, not the launcher's "Loopflow server is not running".
   lf starts itself from some fifty places; stripping the variable at each is
   the wrong direction (`native_session.rs:571` is one such strip).
   - `transport::initialize` takes `LF_OPEN_RECEIPT` out of the process
     environment into a static (`main` is still single-threaded there);
     `answer_open` and `end_open` read the static.
   - The two places that continue the same opening put it on the child they
     start: `start_held`'s environment map (the launcher) and the viewer's own
     re-exec in `relay_with_preview`, if that exec happens before the
     acknowledgement (check; drop this hand-off if it doesn't).
   - Delete `process.env_remove(OPEN_RECEIPT_ENV)` in
     `native_session.rs`: a provider can't inherit what the environment
     doesn't hold.
2. *The tail goes.* `OPEN_TAIL`, `keep_open_tail`, `open_failure`, the
   `CommandExit` branch of `end_open` and the test
   `a_provider_that_exits_unready_is_reported_in_its_own_words` are deleted.
   The acknowledgement is written when the provider process is spawned, so a
   provider that prints an error and exits is already `usable` (qa saw this
   with a stand-in); and a launcher that fails writes its own error first. The
   tail could only answer for a launcher that crashed without returning from
   `main`. `end_open` becomes: a failed command that holds the receipt and whose
   error is not a `CommandExit` answers with that error; the warn line stays.
   Revert the `ANSI_ESCAPE_RE` widening and the `pub(crate)` on
   `strip_ansi_escape_codes` in `provider_auth/mod.rs` unless another caller
   needs them. If `usable` is later made to wait until the provider takes
   input, the tail belongs with that change.
3. *Docs.* `docs/workspaces.md` "If opening fails" says the reason is "lf's own
   error, or the provider's last words if it exited before it was ready". Cut
   the second half; say `usable` means the provider was started.

**Who answers, in order:** the launcher's own error, else the viewer's own
error, else the wrapper's status line.

**Absent/error states:** receipt directory gone (caller timed out and dropped
it) → the write fails silently.

**Tests:** `the_first_answer_to_an_opening_stands` (kept, driving the static);
a unit test that after the take the process environment holds no
`LF_OPEN_RECEIPT`; `a_failed_opening_reports_the_conversations_own_reason`
(kept); the fixture `a_launch_that_found_no_provider_leaves_nothing_to_resume`
still reads the launcher's exact error from the receipt, which now shows the
hand-off through `start_held` works.

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
   its conversation to the end can't answer for its viewer. Section 2 "Changes
   from the second concept review" removes the tail and the ambient variable.
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
- **`a_launch_that_found_no_provider_leaves_nothing_to_resume` depends on the
  Machine.** Choice made for review: the fixture returns early, printing why,
  when `claude` resolves on `os_process::tool_path("/usr/bin:/bin:/usr/sbin:/sbin", <fixture HOME>)`
  (a Homebrew-installed Claude Code). Passed over: a variable that makes lf
  skip a tool directory (a test-only knob in production code), and leaving the
  test to fail on such a Machine.
- **Section 2's three changes** are the next implement pass.

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
- `cargo test -p loopflow --test transport_tests a_launch_that_found_no_provider` — implementer.
- `env -i … lf session resume <throwaway>` — passed in qa, by hand.
- The Rust suites on macOS and Linux — CI; watch fixtures with a bare `PATH`.
- `lf open --task <ID> --json` with the provider missing, on tmux, Rex (`-a pi`) and Herdr — passed in qa. cmux and Desktop not run: gate or a person.

Open PRs on the same files: #1978, #1976, #1967 (`native_session.rs`,
`claude.rs`), #1972 (`server/mod.rs`). Rebase before slice 1 lands.
