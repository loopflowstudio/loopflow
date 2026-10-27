# ce4160 — A taken-over Codex terminal says where its conversation went

Status: built and published (PR #1746); gated 2026-10-27. Based on
main `3fc51be5`, which includes LOO-628 repair PR #1740 (merged); keep its
login-pinning fix intact.

## Problem

Jack's LOO-628 two-login proof (2026-10-27): after `lf --account codex=other@
resume <session>` or a Machine move takes over an open Codex conversation, the
old terminal shows "Reconnecting to server…" then "Reconnect failed — check the
endpoint, then relaunch" and stays open. `moved_away`
(`lf/commands/util.rs`) only prints after the client exits, so it never does.
Claude's client is its own AgentProcess, so takeover ends it and the notice
shows.

Beneficiary: anyone who moves a Codex conversation and glances back at the old
terminal. Accepted outcome (Task brief): the old Codex terminal ends within a
few seconds and prints the notice Claude's does.

## Demo

Two terminals, one disposable Codex conversation:

```
A$ lf -a codex --account codex=<one>   # open, send one short message
B$ lf --account codex=<other> resume <id>
A: within ~3s the Codex UI closes, the shell is clean (no mouse junk, main
   screen back) and prints
     Left “<title>”. Continue with: lf resume <id>  …
     This conversation moved to account <other email>: lf resume <id>
B: the same conversation, history intact, accepts the next message.
```

Machine form: `This conversation moved to <machine>: lf resume <id>` (or
`lf resume --task <ID>` when its Task went with it).

## Risk findings

1. **Codex 0.164 has no signal handler and never restores the terminal.**
   Probe (pty + silent relay socket): SIGTERM and SIGHUP exit in ~10ms, SIGINT
   in ~80ms, by default action; output ends with alt screen (`?1049h`), mouse
   (`?1000/1002/1003/1006h`), kitty keyboard (`>5u`), bracketed paste
   (`?2004h`), focus (`?1004h`) still on and the tty raw. Closing the relay does
   not end it (brief). So lf must stop the client *and* restore its terminal,
   or the notice prints onto an alternate screen that the viewer's exit then
   wipes.
2. **The destination is not known when the old terminal ends.** Today's
   `moved_away` re-derives it after exit, but every mover stops the provider
   *before* writing where it went: `moves::send`/`send_task` call `take()` →
   `stop_session_agent` and only afterwards `complete_moved_session` (and Task
   `hand_off`); an account move (`ops/human_session.rs` `moving` branch) stops
   the old provider and the new launcher pins the new login later. A prompt
   exit would print nothing (Machine) or nothing (account). Claude rides the
   same race today.
3. **The old launcher re-pins its own login after exit.** `run_native_session`
   ends with `route.pin_session_blocking(Some(agent_session))` using the *old*
   route. After an account takeover it can overwrite the new pin. Must not run
   once the attachment is lost.
4. **Race-free detection exists.** `finish_session_attachment` stops the
   provider under the Session's attachment fence, then in one transaction
   writes the `attachment_exit` event and rotates `processes.attachment_token`
   (+ `attachment_exit_seq`). A plain `session_attachment(session)` read that
   no longer equals our `SessionAttachment` is proof we were superseded; the
   exit event reached via `attachment_exit_seq` is the authoritative record of
   why.
5. **Exit-text plumbing already exists.** Every interactive run sits under a
   transport owner (`transport::delegate_current`); `transport/native.rs`
   `NativeClient` replaces the client's exit text with lf's "Left …" line and
   keeps only mode/cursor controls the client printed. It has no path for a
   client that died without restoring.

## Chosen approach

Three pieces, one change.

**A. Movers record the departure with the stop.** `stop_session_agent(store,
session, departure)` takes `Departure`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum Departure {
    Terminal,                                 // same Machine, same login
    Account(ProviderAccountId),
    Machine { machine: MachineId, task: Option<TaskId> },
}
```

`finish_session_attachment` writes it into the `attachment_exit` payload it
already inserts (no schema change). Callers: account move → `Account(target)`
(the login `names_another_account` resolved); takeover, native-resume
selection (`human_session.rs` ~1385) and provider `import` → `Terminal`;
`moves::take` → `Machine { to, task }` (`send_task` passes its Task). A store
read `attachment_departure(session, &SessionAttachment) -> Option<Departure>`
reads that payload by its receipt key `attachment:<token>:exit`, so a later
attachment never answers for an earlier one.

**B. The launcher ends a relay client whose attachment is lost.**
(Revised 2026-10-27 at kickoff to the built mechanism; the earlier
relay-worker `AtomicBool` design is dropped.) `run_native_session` waits via
`os_process::wait_for_exit` and, for a client that does not own its provider
(`command.remote` set — every native Codex client and explicit `--remote`),
ticks a `ClientWatch`: every 500ms it compares `store.session_attachment` with
this launch's `SessionAttachment`; once they differ it SIGTERMs the exact
unreaped child pid, SIGKILL after 2s. A client that owns its provider (Claude)
is already ended by `stop_session_agent`. `serve_codex_native` returns `Ok`
after its loop when the attachment is gone, skipping `ensure_thread_resolved`
and the pending-input bails (the new attachment owns recovery). The launcher
skips its post-exit login re-pin when the attachment is lost (finding 3), and
reports the outcome `interrupted`. Store read errors are logged and never end
the client.

**C. The transport owner restores a terminal its client left behind.**
`transport/native.rs` `NativeClient` observes accepted bytes with the existing
`terminal::AlternateScreen`; the owner captures the pty's cooked termios at
creation. In `finish`, if the client ended while the pty is still raw or on the
alternate screen, the owner restores termios on the master and prepends
`\x1b[<u\x1b[?1004l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?2004l\x1b[?25h\x1b[0m`
plus `\x1b[?1049l` only if alternate is still entered, then the exit line. A
client that restored itself keeps today's bytes. This also cleans up any
crashed native client.

**Notice.** `moved_away(environment)` becomes "read our attachment's
departure": `Account` → `This conversation moved to account <email>: lf resume
<id>`; `Machine` → existing Machine/Task wording; `Terminal` → `This conversation
moved to another terminal on this Machine.`; none → nothing. Shared by Claude
and Codex, printed after the transport's exit line on the restored main screen.

## Alternatives rejected

- *Close the relay / refuse reconnects* — Codex then shows "Reconnect failed"
  and stays (the reported bug).
- *Inject Ctrl+C into the pty* — no restoration either (SIGINT probe), and
  writes into a terminal another viewer may control.
- *Wait after exit for `moved_to` / new pin to appear* — same-login takeover
  never produces either, so the old terminal would hang the full window; still
  racy against `hand_off`.
- *Reorder movers to record `moved_to` before stopping* — a failed move would
  then advertise a destination that never received it; the departure belongs
  with the stop that actually happened.
- *Restore the terminal in the stopping lf* — it cannot see whether Codex used
  the alternate screen (`tui.alternate_screen`, Zellij), so a blind `?1049l`
  can jump the cursor home. The owner sees every byte.

## Scope

- **Source of truth:** `session_events` `attachment_exit` payload (departure),
  linked from `processes.attachment_exit_seq`; attachment token for loss.
- **Surfaces:** native Codex launch (`run_codex_native`, `serve_codex_native`,
  `run_native_session`), `moved_away`/`spawn_session_command_with_env`,
  `stop_session_agent` callers, `finish_session_attachment`, transport owner
  `native.rs`. No wire DTO, Desktop or migration change.
- **Authority:** stop only the client process this lf spawned, by its exact
  unreaped pid, never a process group, another Session or the app-server
  (takeover already stopped that under the fence).
- **Absent/error:** attachment lost with no departure (older mover, reaper) →
  client ends, "Left …" line only. Store read failure → keep serving.
- **Operational:** old terminal ends ≤ 3s after the mover's stop commits
  (500ms tick + immediate SIGTERM; 2s SIGKILL bound).
- **Exclusions:** inline-mode
  (`tui.alternate_screen=never`) is covered by C but not separately demoed.
  Release publication/installation stays with the root Wave conversation.

## Delete — do not maintain

- `type Pin`, `pinned_account` and the before/after account comparison in
  `moved_away`; the post-exit `session_moved_to` + `task_ownership` lookup in
  `moved_away` (replaced by the departure record). `session_moved_to` stays
  for resume-follows-the-move.
- The `pinned` plumbing through `spawn_session_command_with_env`.
- Forbidden: a second "where did it go" path kept beside the departure record,
  a reconnect-refusing relay mode, signalling by group or by provider name.

## Status and remaining work

Built in the worktree (uncommitted): A, B, C and the notice as described;
`Pin`/`pinned_account` deleted; `departure_notice` split from `moved_away` for
the wording test. Integration test
`a_codex_terminal_moved_to_another_login_ends_and_says_where_it_went`
(`tests/session_launch_contract.rs`); its idle client is a one-shot
`idle-terminal` file marker in `native_codex.py` (resumed launches inherit the
captured environment, so an env switch leaked into the mover). The
`FIXTURE_STARTED` idle path stays: `native_runner_is_quiet_until_its_turn_ends_codex_foreground`
uses it.

Finding 2026-10-27: same-login `lf session connect <id> --takeover` while the
first launch's transport owner is live is a *viewer* handoff
(`transport::attach`), not an agent stop; `Departure::Terminal` is written only
when no live owner exists. The Terminal integration variant was dropped; see
`scratch/questions.md`.

Remaining: gate; Machine move acceptance after release (root Wave conversation).

Checks after the `Departure::Terminal` rename (LF_* unset, stdin /dev/null): `a_stop_records_where_the_conversation_went` 1/1, `transport::native` 5/5, `session_launch_contract` 25/25, clippy and `cargo fmt --check` clean; `grep -rn 'Departure::Client' rust/` empty.

## Concept review (2026-10-27, headless)

The model holds: one user moment ("my conversation left this terminal") maps
to one record, the `Departure` written with the stop in `attachment_exit`, read
by the ended attachment's token. Every interactive client (Claude, Codex,
OpenCode) reaches `run_native_session`, so the notice has one path.
`session_moved_to` stays as resume routing, not a second notice source.

Settled and done: `Departure::Client` became `Departure::Terminal` (serde
`terminal`). "Client" already means the terminal UI process in this diff
(`ClientWatch`, `NativeClient`); the payload is unreleased, so the tag is fixed
from release on.

Not reopened: the `Terminal` notice for same-login takeover follows Jack's
steer (cover same-account takeover); a viewer handoff notice stays a separate
question (`scratch/questions.md`). `import` records `Terminal` too; its caller
continues in its own terminal, so the wording holds.

Second pass (after the rename): no further change. `moved_away` reads only
the ended attachment's `Departure`; `ClientWatch` and the transport
restoration carry no product concept of their own. No open product choice;
the viewer-handoff notice stays outside this Task.

## Done when

- `cargo test -p loopflow --lib transport::native` — a client that entered the
  alternate screen and died raw yields `?1049l` + mode resets before the exit
  line and the pty is cooked again; an inline client gets resets only; a
  self-restoring client's bytes are unchanged.
- `cargo test -p loopflow --lib a_stop_records_where_the_conversation_went` —
  each `Departure` is returned for the ended attachment and `None` for the new
  one.
- Integration: old Codex `lf` exits 0 within 5s of an account move, prints the
  account notice, its fixture client is gone, the new client holds the same
  thread and the new pin survives.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- Real proof on this Machine: disposable repo, real `codex` 0.164 in a pty,
  same-login takeover ends terminal A within ~1s with restored modes and the
  notice; account move if a second Codex login is already registered (no new
  credentials or route changes). Machine move: installed acceptance remains
  for the root Wave conversation after release (no rentals).

## Check results

- 2026-10-27: focused lib tests 7/7, `session_launch_contract` 25/25, clippy
  `--all-targets -D warnings` clean, fmt applied.
- 2026-10-27 real proof (dev build, store copy, codex 0.164, pty): A on
  jackstah@gmail.com, B `--account codex=jack@loopflow.studio resume` — A exited
  0 2.7s after B started, emitted `?1049l` + mode resets, the "Left …" line and
  the account notice; B resumed with history; pin moved to B's login.
- 2026-10-27 compress: focused lib tests 9/9 (incl. the Codex planning test the
  fixture edit had broken), `session_launch_contract` 25/25, clippy clean.
- 2026-10-27 realign: no code change, no rerun; origin/main has nothing past base.
- 2026-10-27 rename to `Departure::Terminal`: `a_stop_records_where_the_conversation_went` 1/1, `transport::native` 5/5, `session_launch_contract` 25/25, clippy clean, fmt applied; no `Departure::Client` left in `rust/`.
- 2026-10-27 publish (after syncing main e970aec6): clippy/fmt clean, focused lib 6/6; `session_launch_contract` 23/25 in-suite at load avg ~200 (opencode setup-failure and machine-dispatch 20s deadlines), both pass alone.
- 2026-10-27 gate (load avg 210–250): `cargo fmt --check` and clippy `--all-targets -D warnings` clean; focused lib 8/8; `session_launch_contract` 23/25 in-suite, the same two as at publish (opencode setup-failure 20s deadline; machine-dispatch `success` assert), both pass alone (20.9s, 57.4s). Neither touches the Codex move path; the only shared change is a 50ms `wait_for_exit` poll. CI owns the in-suite result.
