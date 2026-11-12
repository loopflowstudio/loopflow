# Simplification Opportunities

Area: where an agent runs (`transport/`, `server/holder`, `os_process`,
`desktop/shells`, `engine/launch`) and the docs that describe it. Read on
2026-11-12 against the working tree mid-implementation (rollover already
gone, `delegate` already calling `os_process::detached::run`).
`docs/dev/index.md` now has "What runs on a Machine"; this file is what the
code still carries beyond that section.

## Product intent

An agent runs in one of three places: **here** (the launching command's
child), **held** (a PTY the holder keeps, seen through views the server
relays), or **detached** (under a Graph process, seen through its log). Three
processes stay up: server, holder per build, Graph process per running Graph.
The owner (`lf session serve`) was a fourth place that did all three jobs for
one conversation at a time; everything below is what still has its shape.

## Opportunity 1: `Host` has one variant

**Misalignment**: `transport::Host { Owner(LfProcessId), Terminal(TerminalId) }`
and `session::Holder { Caller, Owner, Server }` both keep a middle case for a
process that hosts one conversation. After this Task nothing creates one.

**Symptom**:
- `transport.rs` (3,277 lines) is an owner (`start`, `serve`, `Spool`,
  per-owner sockets from `endpoint`, `--terminal`, `transport.log`) and a
  viewer in one file. `hosted.rs` (1,285) puts `Child` in front of a PTY, a
  pipe or a held terminal so `serve` and the registry can share it.
- `wire.rs` counts a viewer's stream two ways: an owner's viewer counts
  snapshot/out/err frame bytes against the owner's spool, a held viewer
  counts PTY bytes. `reconnect` and `reconnect_held` are the two recoveries.
- `Host::Owner` is matched in 31 places across `terminal_title`,
  `ops/human_session`, `ops/task/retirement`, `lf/commands/open`,
  `server/service`, `desktop/shells` and the store.
- The transport README describes the owner for 315 lines, then says held
  terminals are the same "with 'the server' in place of 'the owner'".

**Realignment**: a host is a `TerminalId`. Delete `Host`, `Holder::Owner`,
`start`, `serve`, `Spool`, the frame-counted stream and `Child`'s PTY and pipe
arms; `hosted.rs` becomes the registry's per-terminal state. The README starts
from held terminals.

**Cascade**: `session attach` takes a terminal only; `lf session serve`, its
`--terminal` relay, `shell_history(owner)`, `leaving(owner)` and the owner
half of `transport_tests.rs` (9,361 lines) go; retirement asks the server to
end a terminal where it now signals a verified owner process.

**What decides how much of this lands now**: viewing owners that an earlier
build left running. `scratch/questions.md` assumes the viewer side stays
(`lf session attach OWNER`, `session.attach {owner_lf_process_id}`), which
keeps `Host::Owner`, the frame-counted stream, `reconnect` and the per-owner
socket: most of the list above. Dropping it leaves those conversations
running and unviewable, and `lf resume` would start a second agent on the
same thread. Three ways through:
1. Keep the viewer half for one release, marked with the Task that removes it.
2. `lf resume` of an owner-hosted Session ends that owner and resumes the
   thread held. The agent restarts; the conversation continues from provider
   history. Costs one interrupted turn per old conversation, once.
3. Delete now and say so in release notes: quit old conversations before
   installing.
The brief says "Existing owners already running on Machines keep working
until their agents quit; no migration of live processes." (2) ends a live
process on resume, so it needs a yes.

Answered, Jack Heart, 2026-11-12, on whether `lf resume` should still view
owner-hosted conversations: "not necessary any more. ideally it would just
restart the session". So (2): no viewer half; resume restarts the Session
held. This replaces the brief's "no migration of live processes" for resume.

## Opportunity 2: two more launches still ask for an owner

**Misalignment**: the brief names Desktop shells as the last owner caller.
`delegate` has another: `desktop_relay()` is true when `LF_SERVER_STATUS` or
the framed-stream variable is set, and a launch that is not Work then falls
through to `start(...)`. That is a bare Session opened in a Desktop pane or
over a framed `lf --machine` view. `Holder::Owner` exists for exactly this
("a Desktop shell's relay, a framed `lf ssh` viewer").

**Symptom**: `if !desktop && (!work || …)`, `if !held && !desktop`,
`Host::Owner(start(...))` in `delegate`; `transport/connection.rs` (410
lines, the **Opening… Input paused** handshake); `LF_SERVER_STATUS` in
`registry.rs` and `os_process.rs`.

**Realignment**: a pane or a framed stream is already a view, and a view has
no terminal for the agent to be the foreground program of. So the terminal
question becomes "is something viewing this through a relay or a tty", and a
bare Session opened that way is held like any other viewed conversation. Its
hooks stay bare: hooks follow Work, the place does not change them.
Not verified: whether Desktop needs the turn hooks `Holder::Owner` adds to
know a bare pane can take a reply, or whether the terminal's status reports
already carry that. If it needs them, that is the piece to name in the PR.

**Cascade**: `delegate` is a three-way match with no `desktop` flag;
`Holder` is `Caller | Server`; `connection.rs` loses its owner
acknowledgement and is deletable with the "every view types" piece of
`docs/dev/server-first.md`, which is not this Task.

## Opportunity 3: viewing is recorded on a Process row

**Misalignment**: who is viewing a conversation decides when a finished
Task's checkout and conversations are cleaned up. That fact is kept on the
host's Process (`processes.viewer_hold`, `viewer_hold_at`; Sessions point at
it through `transport_lf_process_id`) because the host used to be a process.
Held terminals record the same thing on `held_terminals`.

**Symptom**: `transport/lifetime.rs` (414 lines) writes viewing off the byte
path for owners; `store/sqlite/transport.rs` translates between the two
shapes; `task_retirement.rs` reads both; 58 references to `viewer_hold`, 32
to `transport_lf_process_id`.

**Realignment**: viewing belongs to the terminal. Readers take it from
`held_terminals` only; the Process columns stop being written.

**Cascade**: `lifetime.rs` shrinks to the explicit shell Close;
retirement has one query. The columns go by the staged rule: stop reading
now, contract migration after the window, filed as its own Task.

## Opportunity 4: four documents, three tenses

**Misalignment**: `docs/dev/server.md` still says "Session owners own
providers, PTYs, terminal output and input/geometry claims" and describes
**Move here**; `docs/dev/server-first.md` describes the same system as a plan;
the transport README describes the owner and then amends it;
`docs/dev/reference.md` rows (LfProcess, Client API server, Desktop shell)
name `lf session serve` and "existing PTY owner".

**Realignment**: `index.md#what-runs-on-a-machine` is the model. The others
link to it and keep only what they own: `server.md` client admission and
installation; transport README the holder and view protocols; `reference.md`
the checked inventory. `server-first.md` keeps its telemetry section, the
pieces still unbuilt ("every view types", and viewer cleanup if option 1
above is chosen) and its open questions; "Two kinds of Session", "Processes"
and "The managed rule" are now current and move out.

**Cascade**: one place to change when the takeover model goes.

## Not this Task

The control model (one viewer holds an input token, **Move here**, Busy,
Ctrl+] detach, the `LF_SERVER_STATUS` handshake) was built for a conversation
owned by one terminal. `server-first.md` plans "every view types; size
follows the newest view". It is the largest remaining cascade and a product
change people will feel, so it wants its own Task and demo.

## Aligned areas

- The holder: no store, no Sessions, five verbs (start, write, resize, read
  from a byte position, report exit). Small enough that a server talks to
  holders from older builds.
- `os_process::detached`: one pipe, an id line, `released`; "no line means
  delivery unknown, do not replay". Graph launches, landing watchers and ship
  all go through it.
- Server restart rebuilds from holders, per-terminal files and the store;
  the terminal directory is the exit receipt.
- `transport::delegate` as the single place a launch's place is chosen.
