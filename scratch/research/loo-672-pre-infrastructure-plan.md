# Historical kickoff plan — superseded November 4, 2026

This file preserves earlier reasoning, not current instructions. The accepted
design is `../loo-672-ready-conversations.md` relative to this research directory.

# LOO-672: Task conversations ready in Desktop without clicking Open

Status: **draft** (kickoff 2026-10-28). Jack Heart's Task brief is the accepted
product direction; the mechanisms below are this plan's choices.

## Problem

Jack Heart clicks **Open** to see a Task's conversation after launching it, after
selecting it, and again for every Task after a reboot. When another terminal takes
a conversation, the pane turns into a placeholder with no clear way back. Keeping
everything live has to stay within the machine's memory.

## Goals (Jack Heart, 2026-11-02, word for word)

(1) We are using everything possible we can learn from Rex on how to make an efficient workflow tool based on terminals sessions (in our case mostly LLM agent sessions)
(2) Loopflow should be helping you be able to multiplex across many agent sessions on your machine and, overall, gaining you more from smart management of those sessions than it is adding overhead in terms of memory
(3) we ultimately want to be able to have very fast switching between tasks that you are actively working on or tasks that are open on your desktop
(4) we want you to be able to close any sessions that you don't care about or let them go idle and have them not cost you from a performance perspective

These replace the draft's "stated budget" framing. The measure is net: smart management
must gain more than lf's own overhead costs (LOO-708 owns lf's process overhead).
Evidence on 2026-11-02: 48 Claude processes held 10.2 GB private memory; 39 were
interactive and idle, 22 of them with no viewer attached (about 5.2 GB).

## Direction (Jack Heart, 2026-11-02)

Jack: this Task, "the kind of user experience task, kind of fights against those goals.
But I think we should set up the infrastructure and then figure out how we want to
build a good user experience based on what we've had." LOO-672 therefore waits at
explore while the infrastructure lands, then its Approach is redesigned on what exists:

- LOO-708 (Infrastructure, running): lf's own per-conversation processes stay small.
- LOO-709 (Infrastructure, running): an idle conversation costs nothing; return paints
  the last screen; `resume --all` restores only the viewed set.
- LOO-710 (Infrastructure, running): session snapshots at Rex fidelity (libghostty-vt).

Jack also asked for a second deep dive into Rex's architecture and libghostty, and for
continued research into where Loopflow's user experience and system architecture were
designed in a problematic way. Findings go below as they arrive.

## Demo

1. Relaunch Desktop. Click through 10 Tasks in the navigator. Each shows its
   conversation live, or "Resuming…" over its last screen and then live. No Open
   click.
2. In Terminal, run `lf resume LOO-672`. The Desktop pane dims its last screen under
   an overlay: "Continued in another terminal on this Machine (cmux · workspace
   loo-672)". It offers **Take back** and **Fork**, with the equivalent `lf` command
   under each. Take back brings it back, and the Terminal client prints where it
   went. Fork opens a new conversation here, and Terminal keeps running.
3. Simulate the reboot: kill `-9` every owner and agent, then relaunch Desktop. The
   Tasks resume with no error from their dead PIDs.
4. Visit 20+ Tasks, then run `footprint` on Desktop and count provider processes.
   Both stay inside the budget below. Returning to the first Task resumes it and
   shows its history.

## What exists (findings, 2026-10-28)

- **Opening is click-only by design.** `WorkModel.revealPrimaryConversation`
  (`swift/LoopflowMac/WorkModel.swift:217`) skips Tasks. Selection never calls
  `SessionsStore.select`. Panes with dead owners show "Not running here" with a
  Resume button (`WorkspacePanesView.swift:777-834`). The only open paths are
  `openTaskConversation` (button) and `attachTaskConversation` (Dashboard).
- **The restart machinery mostly exists.** Desktop runs `lf resume --all` at launch
  (`LoopflowApp.swift:43`). It resumes Sessions whose owner never recorded an exit
  and is gone from the OS (`ops/human_session/restore.rs`), each in a viewerless
  owner. Stale evidence already reads as dead:
  - `began_before_boot` uses `kern.boottime`.
  - A PID whose start time differs by more than 3s counts as dead
    (`journal/os_process.rs:76`).
  - Unknown evidence never authorizes a stop.

  What is missing after a reboot is the per-Task click. Restored panes stay
  `.pending` until Resume.
- **Opening can start a turn.** With no captured input or native history,
  `open_waiting` (`ops/human_session.rs:975`) appends input and launches the primary
  message. Auto-open must never reach it.
- **Viewer takeover is silent.** A controlling `Attach{replace}` on the same owner
  revokes the old viewer's token. The old viewer's input is dropped
  (`transport.rs:634`, `_ => {}`), with no message and no exit. Provider takeover
  (`--takeover`) is different: it stops the agent and writes an `attachment_exit`
  event with a `Departure`, and the old `lf` prints "moved to another terminal".
  Desktop learns of either only when its child exits, then *replaces* the terminal
  with the "Active in another terminal" placeholder (`WorkspacePanesView.swift:836`).
  There is no Fork anywhere.
- **Fork primitive.** `claude --resume <id> --fork-session` exists in the installed
  Claude Code. Codex rejects `thread/fork` (`harness/codex_connection.rs:500`).
  LOO-618, which decides the `lf resume`/`--fork` semantics, is unstarted.
- **Memory is unbounded.** Measured on Jack's Machine at 2026-10-28 13:00:
  - Desktop footprint was 2.5 GB after 11 minutes, with about 15 connected surfaces.
    The largest categories were MALLOC_SMALL 1.24 GB, MALLOC_LARGE 0.8 GB and
    IOSurface 257 MB. The surface pool has no cap or eviction, and scrollback is
    `10000000`.
  - 82 Claude processes totalled 24.3 GB, with RSS p50 346 MB, p90 516 MB and max
    808 MB.
  - Each `lf session serve` owner is about 30 MB.
  - There were five `lf monitor work --watch` processes at about 170 MB each. They
    are not this Task's scope, and no Task has been filed for them yet.

  Agents dominate. Unviewed owners already end after
  `session.end_detached_after` (default 24h, `transport/idle.rs`) through
  "record exit, `lf resume` reopens from provider history". A conversation that
  ended this way is excluded from `resume --all`. That is already the "suspend"
  primitive; it only lacks a count bound.

## Approach

One rule for the selected Task, in Swift `SessionsStore`, driven by Rust facts:

| Rust says (SessionRecord) | Desktop does on selection, launch or poll |
|---|---|
| Local surface exists | show it (unchanged) |
| Owner running, no controlling viewer elsewhere | attach `--if-free` (today's `select`) |
| Owner running, held elsewhere (`move_here`) | overlay, nothing prepared |
| Owner gone, `opens_turn == false` (native history) | resume in Refuse mode: "Resuming…" over the last screen |
| `opens_turn == true` (nothing to resume) | existing Start / New conversation offer; click only |
| prepare failed | existing failed state + Retry |

The rule applies only to the **selected** Task's conversation pane. Dashboards,
prefetch, cache restore and unselected Tasks never prepare anything. Launching a
Task from Desktop leaves it selected, so the next poll that shows its Session
attaches it. A launch from `lf` elsewhere shows up when Jack selects the Task.
Interactive launches in another terminal show the overlay.

### Rust changes

1. **`opens_turn: bool` on `SessionRecord`.** This is a new required field. It is
   true when `open` would reach `open_waiting`'s append or fresh launch, and false
   when it resumes native history or attaches. Desktop's auto path requires
   `false`. Add the field to the DTO fixtures (`session.json`, `sessions.json`,
   `session_page.json`, `session_states.json`) and to the Swift mirror.
2. **Viewer replacement announces itself.** When a controlling attach replaces
   another viewer, the owner sends the old viewer `Response::Replaced { holder }`.
   The old `lf session attach`/`connect` client then prints the same departure
   notice as provider takeover and exits 0. This makes both kinds of takeover look
   the same to Desktop: the child exits with a notice.
3. **`held_by: Option<String>` on `SessionRecord`.** This is a human label for where
   the controlling client is, Rust-owned. It comes from the attached LfProcess's
   `terminal_id` / `host_placement` and the Machine label. Examples: "another
   terminal on this Machine", "cmux · workspace loo-672", "Machine mini". It is
   `None` when unknown. Unknown is shown as "another client", never guessed.
4. **`--fork` on `lf resume` / `session connect`** (`--prepare --json` supported).
   It creates a new LfSession bound to the same Task, with the source's
   provider/account. It launches `ProviderInvocation::Resume` with Claude's
   `--fork-session`. The source keeps running untouched, and history records the
   fork's source Session. A provider that can't fork returns an error naming the
   provider; the `Open` action carries `unavailable_reason` for fork.
   LOO-618 keeps its scope: making takeover the default, `session new`, and
   deleting `--try/--takeover/--if-free/--fresh`. It inherits `--fork` unchanged.
5. **Count-bounded detached owners.** Add `session.keep_detached` (default 8, `off`
   disables the count). When an owner becomes unviewed and is not among the 8
   most recently viewed unviewed owners that wait for a person, it ends after a
   2-minute grace instead of `end_detached_after`. It uses the same
   `owner_waits_for_person` predicate (no turn, Graph or headless Run) and the same
   self-signal path. Ended owners recorded their exit, so `resume --all` after a
   reboot restores at most the viewed owners plus 8 detached ones. This bounds both
   steady state and reboot. Reconcile the README's "every owner judges all owners"
   sentence with the per-owner code in `idle.rs` while there.

### Swift changes

1. **Selection reveals the Task conversation.** Drop the `.task` exclusion in
   `revealPrimaryConversation`, and route Tasks through the existing
   `prepareLinkedTask` preparation without navigation side effects. After
   `reconcile`, re-run the rule above for the selected Task's Session whenever its
   item is `.pending`/`.ended` and the record changed. This covers launch, external
   start and reboot.
2. **Overlay replaces the placeholders.** When the child exits with a notice or the
   record shows `move_here`, keep the pane's last screen. The 16 KB excerpt that
   `handleSurfaceClose` already reads is rendered read-only and dimmed under a
   noninteractive overlay. The overlay shows `held_by`, **Take back** (today's
   moveHere preparation) and **Fork** (the `--fork` preparation, which replaces the
   pane's Session id). Each shows its equivalent command: the prepared `open_argv`
   with the env prefix stripped, so it can't drift from the CLI or across LOO-618.
   No click, no takeover.
3. **Surface cap.** The `GhosttySurfacePool` keeps at most 8 Session surfaces per
   window registry, ordered by most recent selection. Shell tabs and the visible
   panes are exempt. Releasing the least recent one detaches its viewer: the owner
   lives on, and Rust's count bound may end it later. Before release, the pool
   stores the last-screen excerpt. Returning attaches the surface or resumes it
   under that excerpt. The provider re-renders the conversation history on resume.
4. **"Resuming…" state.** `.opening` shows over the excerpt, not a blank pane.

### Memory budget (stated; slice 1 confirms the numbers)

With 20+ Tasks visited in one window, Desktop holds at most 8 Session surfaces.
Desktop-viewed plus detached provider processes number at most 8 + 8. Targets:

- Desktop footprint at most 1.5 GB.
- Agent RSS sum at most 16 × p90 (about 8 GB) in the worst case, and about 3 GB
  typical.

The earlier measurement is the "before"; slice 1 records the "after". Jack Heart
retired numeric performance targets on October 7. This budget exists because this
Task's brief asks for a stated one.

## Alternatives considered

- **Suspend by SIGSTOP** (keeps the process and terminal state). Rejected: it frees
  no RSS, and a stopped process still holds its PTY and claim.
- **Desktop ends owners itself** (new `lf session suspend`). Rejected: it adds a
  second ending path and a command. The CLI-detached owners would stay unbounded.
  The count bound belongs where the 24h limit already lives.
- **Drop `resume --all` and resume lazily on selection only.** Rejected:
  `resume --all` is already bounded once ended owners record exits, and it makes
  "live" (not just "resuming") the common post-reboot state for recent Tasks.
- **Wait for LOO-618.** Rejected: Jack wants these four improvements together.
  `--fork` is additive, and the overlay displays Rust's argv, so LOO-618's flag
  cleanup changes no Swift.
- **Retain the dead Ghostty surface under the overlay.** Rejected: its PTY is gone,
  and it costs the full surface memory. The text excerpt is enough to orient.

## Scope

**Exclusions:**
- Dashboard tiles stay click-to-open.
- Remote-Machine Sessions follow the same rule but get no new transport work.
- The duplicate `lf monitor work --watch` processes go in a separate Task.
- LOO-618's flag deletions and `session new`.
- Codex fork (offered once Codex supports it).
- Exact scrollback restoration beyond what provider resume re-renders. Jack Heart
  (2026-10-28) mostly wants to land at the bottom when switching back, so returning
  to a Task scrolls to the bottom, and releasing a pane loses nothing he relies on.

**Absent/error states:**
- `held_by` unknown → "another client".
- Fork unsupported → Fork shown disabled with the provider's reason.
- Resume refused (claim not dead, Unknown evidence) → the existing failed state
  with Rust's reason and Retry; never a takeover.
- Prepare failure on the selected Task never touches other Tasks' surfaces.

**Forbidden outcomes:**
- Any auto path that prepares `--takeover`, `--try` or `--fork`.
- An auto path when `opens_turn` is true.
- A second "suspended" store or state in Rust.
- Swift-side guessing of where a conversation went.
- A Swift copy of the CLI command text.

## Delete — do not maintain

- The `.task` exclusion in `WorkModel.revealPrimaryConversation`
  (`WorkModel.swift:217-228`).
- The `_sessionPlaceholder` "Not running here" + Resume branch for resumable
  Sessions, and `_elsewherePlaceholder` "Active in another terminal"
  (`WorkspacePanesView.swift:777-863`). "Move here" in `SessionSlotOffer`
  (`:72-116`) becomes the overlay's Take back. Keep the failed state and Retry.
- `SessionsStore.resume(_:)` as a click target. Its preparation becomes the auto
  path.
- The silent `_ => {}` drop for replaced viewers in `transport.rs:634`; stale and
  passive viewers stay ignored.
- Tests asserting click-only opening must flip, not be duplicated:
  - `SessionsStoreTests.recoveryButton` / `resumesOnlyTheSelectedSession`
  - `WorkNavigation`/`TaskGraphProof` "Task navigation opens Work only"
  - `WorkDestinationTests.taskConversationActiveElsewhere` (Move here wording)

  Keep "unstarted Task's slot offers Start; showing it starts nothing".

## Internal slices (one PR)

1. **This slice: Rust facts.** Add `opens_turn` and `held_by` with DTO fixtures in
   Rust and Swift, and `Response::Replaced` with the departure notice. Focused
   test: in `transport_tests.rs`, a second controlling attach makes the first viewer
   exit 0 with the notice, and `held_by` names the new terminal.
2. **Rust bounds and fork.** Add `session.keep_detached` and `--fork`, with tests in
   `session_lifecycle_tests.rs`.
3. **Swift auto rule, overlay and surface cap**, with tests flipped as listed.
4. **Measure** the after-budget and record one line here.

## Done when (gate, headless)

- `cargo test -p loopflow --test integration -- session_lifecycle transport session_launch_contract`
  passes, and includes:
  - A pre-boot owner and agent with reused PIDs do not block `resume --all` or
    `connect --prepare`.
  - The 9th unviewed waiting owner ends after its grace, while one holding a turn
    survives.
  - `--fork --prepare` returns a new Session with `--fork-session`, and the source
    stays running.
  - `opens_turn` is true only for a Session without native history.
- The DTO fixture round-trip tests pass in Rust and Swift.
- The Swift suite via `uv run python scripts/test.py --reuse-passing` passes, and
  includes:
  - Selecting a Task whose Session has history and a dead owner reaches `.prepared`
    with a non-takeover argv.
  - `opens_turn == true` stays offer-only.
  - A `move_here` record shows the overlay with both commands and prepares nothing.
  - Selecting a 9th Task releases the least recent surface, and reselecting it
    resumes.
- Demo/review (Jack): the four demo steps on the real app, plus a `footprint` line
  against the budget.

Check results: none yet.

## Rex study (2026-11-02, Rex Beta 0.1.0 build 1696, read from the running install)

Jack Heart leans toward vision C (never see "Resuming") and asked to study Rex first.
Evidence: `rex --help`, the API guide compiled into the server binary, and
`~/Library/Application Support/rex/` (the snapshot SQLite was copied, never edited).

- **One server owns every terminal.** `rex server run` (launchd-parented, ~57 MB RSS
  with 8 terminals) holds the PTYs and a headless libghostty terminal per block, with
  about 64 MiB of scrollback each. Rex.app (~400 MB) is only a client. "The server keeps
  a session running whether or not anything is looking at it." Limits are refusal caps
  (`--max-sessions 64`, `--max-blocks 1024`), not eviction: nothing is ever suspended.
- **Attach = snapshot + stream.** A block's stream starts with a Ghostty-format snapshot
  of the terminal, then live output. Switching is instant at full fidelity because the
  server already has the rendered state. The app also keeps a sealed per-terminal
  `.snap` (~10 KB) for first paint.
- **Restart = layout plus fresh shells; commands are never replayed.** `snapshot.db`
  stores the session layout and, per block, its creation options
  (`{command, cwd, path_prepend}`), size, and the process's last cwd (`state.json`).
  Tested 2026-11-02 on a throwaway server (`--state-dir`), after both a clean stop and
  `kill -9`: every block came back in place as a new login shell in its last folder,
  and the log says `terminal: not replaying creation options on restore dropped=[command]`.
  A command that had exited without `--keep-open` had already closed its block. A
  still-running command and a `--keep-open` one each ran once in total. The old screen
  and scrollback were not restored either. So Jack's Rex block for `lf session resume
  LOO-705` comes back after a reboot as a shell in the LOO-705 worktree, not as the
  conversation.
- **Many clients, no takeover.** Every attached client sees the stream and can send keys.
  One client may claim **owner** (`session.own_block`), which only decides the terminal's
  size and colors. A dropped client stays attached for 60 s before `client_disconnected`.

What this means for LOO-672:

- lf already has Rex's attach half. `lf session serve` owners keep a vt100 snapshot
  (text grid, 2,000 scrollback rows; `transport/README.md`) and repaint a new viewer
  from it. Lower fidelity than libghostty, but the same shape.
- The difference is the process count: 56 owners at about 30 MB each, versus one server.
  And Rex leaves the agent-memory problem alone: it would keep all 82 Claude processes
  running. Our budget has to come from ending idle agents, which Rex never does.
- Rex's restore deliberately runs nothing: it rebuilds places, not programs. lf does
  more, because a Session knows what is safe to continue: `lf resume --all` resumes only
  conversations with native history, without replaying input or interrupted turns.
- Rex's multi-client model answers the takeover overlay differently: Desktop would keep
  watching and could type, with no Take back needed. That conflicts with LOO-618's
  takeover default for viewers, and is still open.

### Snapshot quality (2026-11-02)

Jack Heart asked whether lf can match Rex's snapshot quality. Rex's "Ghostty snapshot
format" is upstream libghostty-vt, and the Ghostty revision Desktop already pins
(`a60e9e2`, `scripts/loopflow-dev.py`) ships it:

- `include/ghostty/vt/snapshot.h`: `ghostty_snapshot_encode` / decoder. A CRC-checked
  `GHOSTSNP` stream with terminal state, active screens, unfinished VT input, a READY
  marker, then scrollback newest-first so a client can render before history arrives.
- `include/ghostty/vt/formatter.h`: VT output of the screen and scrollback that can
  carry hyperlinks (OSC 8), Kitty keyboard state, palette, modes, scrolling region,
  tabstops and pwd. These are the things `transport/README.md` lists as missing with `vt100`.

Path: the owner (`transport/terminal.rs`) feeds PTY output to a libghostty-vt terminal
instead of `vt100::Parser`, and repaints a viewer with the VT formatter. Desktop's
viewer is already a Ghostty surface, so it shows exactly what the owner had. A snapshot
written at owner exit would also let a pane show the real last screen after a reboot.
Cost: the cross-platform `lf` binary links a Zig-built static library (Zig is needed
in CI and release for Linux and macOS, or as prebuilt per-target archives), plus an
`unsafe` FFI wrapper. Undecided: whether this belongs in LOO-672 or its own Task.

### lf's own process overhead (measured 2026-11-02, private footprint, not RSS)

About 150 `lf` processes held about 5.5 GB private memory: viewers (`session connect`/`resume`)
53 × ~32 MB, `lf -i` launchers and their owners ~23 × ~73 MB, batch steps 11 × ~42 MB,
polls and other commands ~49 × ~29 MB. A sample of one launcher showed it idle in
`session_command_status_with_env` (a sleep loop waiting for Claude to exit) while
holding 68 MB MALLOC_LARGE and 36 MB MALLOC_SMALL left over from launch. A Desktop pane
that launched its conversation runs `connect` → `serve --terminal -- connect` →
`connect` → `lf -i` → `claude`, because the viewer that starts the agent is itself
delegated to an owner (`transport.rs` `start`). Candidate reductions are in the reply
of 2026-11-02; none has been chosen yet.
