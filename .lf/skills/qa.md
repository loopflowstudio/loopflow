---
requires: a change or behavior to assess
produces: a change that works for the people who will use it, with what you saw and what is still wrong
action_style: procedural
---
Use the change the way the people it is for will use it, and fix what breaks.

You are its first user. Nobody should open a demo or merge a PR and find it broken
on the first click; if they would, this step failed.

## Use the real thing

Run the real product, not its tests: the built app, the installed CLI, the live
page in a browser, the API over the network, the simulator or device. Drive it
with computer use, a browser, a terminal or whatever reaches it the way a person
would. Internal setters, test-only endpoints, mocks and fixtures do not count.

Getting there is your job. Build it, start its services, create accounts and
realistic data, sign in, set up whatever state a real user would already have.
When no route exists, make one. Hand back only what truly needs a person, such
as their own credentials, a physical device, a payment or a production approval,
and say exactly what that left unchecked.

Make the scenario as real as you can reach:

- Arrive the way a person arrives: a fresh install or first visit, then a
  returning user with existing data from before this change.
- Do the main job end to end, then the things people actually do: wrong input,
  empty and large data, going back, interrupting and resuming, a slow or failed
  network, two things at once.
- Check what it leaves behind as well as what it shows: files, rows, messages,
  processes, and what another part of the product now reads.
- Look at it. Read the output, check the screenshot, notice what is confusing,
  slow or ugly, not only what is wrong.

## Loopflow's local server

Build `cargo build -p loopflow --bin lf` and use that binary for branch QA.
For write scenarios, use a temporary Git repository with a local Git identity,
an isolated `HOME` and `LF_HOME`, and no inherited `LF_*` caller variables.
Set `default_agent: codex` in its `.lf/config.yaml`. Create real records with
`lf task create --title ... --json` and `lf session resume --prepare --json`;
`--prepare` creates connection data without starting a provider. For an upgrade
scenario, prepare a Session with the installed binary before opening the same
state with the branch binary.

Run `lf server run` as a child with closed stdin; get its socket from
`lf server status`. Use the revision-2 newline JSON protocol in
`rust/loopflow/src/server/README.md` over that Unix socket. Exercise Swift
encoding with the built `Loopflow` library's `ServerClientMessage` and
`ServerOperation`; this checks live interoperability, not Desktop UI behavior.
Check the resulting state through the CLI as well as the socket. Dashboard
queries accept `task:<id>`; hide/show members use that same selector.
`lf desktop layout query:<id>` already prints JSON and takes no `--json` flag.
Session pages return `entries` and `next`. Stop and reap every server child,
including on failure; no service installation is needed for this isolated run.

## Launching Loopflow Desktop

The snapshot and quit probe below needs a display; use the headless checks above
when WindowServer is unavailable.

- Build the CLI and local server: `cargo build -p loopflow --bin lf`.
- Build the debug app: `swift build --package-path swift -Xswiftc -gnone`
  (a few seconds when the build cache is warm). Binary: `swift/.build/debug/LoopflowMac`.
- `uv run python scripts/check_desktop_launch.py` launches it with
  `-ui-test-mode empty-workspaces`, waits for the snapshot and the quit, and
  prints the sampled stacks if it hangs. That mode renders fixture data and
  reads no registry, so it runs safely beside the person's live Loopflow.app.
- To look at the frame, call `check_launch(executable, dir)` from that script
  with a directory you keep. `snapshot.png` is written there.
- `scripts/release-loopflow.py` parses no flags. Even `--help` starts a full
  release build.

## Fix and repeat

Repair what breaks within the change's scope, then go through the experience
again from the start. Keep going until it works for the people it is for. A
change to what the product should be is not yours to make; describe it
instead. When asked for a read-only review, report instead of
fixing.

Run tests only where they tell you something the real path cannot reach, and
keep any you add about what a person would notice.

## Loopflow server

Build the branch with `cargo build -p loopflow --bin lf`. Start that binary's
`server run` with an isolated `LF_HOME`, after removing inherited `LF_*` from
its environment. `server status` reports its socket. This leaves the installed
service alone; stop and reap the foreground server when finished.

Drive newline-delimited JSON over that socket, using the hello, subscription,
request and checkpoint shapes in `rust/loopflow/src/server/README.md`.
For returning-state checks, a consistent SQLite backup of the existing store
keeps real Task records without writing the original database. Never seed rows
through internal APIs. Use only reads against that backup, keep probe files in
the current checkout, and restore its prior file bytes and index entries.
Comparison bases accept `parent`, `head` or a resolved commit SHA, not `HEAD`.

## Loopflow served launches

Build with `cargo build -p loopflow --bin lf`. Use a disposable Git repository
and separate `HOME` and `LF_HOME`; clear inherited `LF_*`, then set `LF_BIN` to
the absolute branch-built binary. Start that binary's `server run` there and
read the socket from `server status`. Never fault the installed Machine's store.

Connect over the Unix socket with newline-delimited JSON: send the revision-2
hello from `tests/fixtures/dto/server/hello.json`, then requests shaped like
the adjacent `graph_run.json` or `task_run.json`. A local Graph containing
`- cmd: help` exercises real execution without a provider. For disposable
Tasks, use a local bare Git origin, `planning: {provider: git, personal: true}`
and `graph: qa-normal` in `.lf/config.yaml`. Commit and push those definitions,
then `lf task create --title "Launch QA" --json` creates local planning data.
Create with stdin closed; `--graph` on creation requires `--run`.

To delay a report, SIGSTOP the launched command before it reports, leaving its
execution owner running. Check that a live owner waits beyond the unknown grace.
In this disposable home only, make the registry unavailable and restore it;
check recovery both before and after the uncertain reply. Resume the command
and inspect its actual result without launching it again. Find completed direct
Graphs with `lf graph list --processes`, then `lf process show <ID>`; the live
process list cannot find a finished run. Stop the private server and every
remaining child after the scenarios.

## Keep what you learned

How to launch and drive this product belongs in this skill. When you had to work
out a step it does not describe, or a step it describes no longer works, write
the working version here with `lf skill customize qa`, replacing what was wrong.

Show what you did in `scratch/qa.html`: each scenario, what you did, what you saw
with screenshots or transcripts, and what you fixed. Finish with what works now,
what you could not reach and why, and what is still wrong, worst first. Do not
publish, land or choose the next Graph step from here.

## Loopflow Desktop

For Desktop in a headless run, follow `swift/README.md`'s Testing section:
`scripts/test_desktop.sh --filter '<affected suites>'` builds the app and CLI,
then runs model and production-view checks with WindowServer access denied.
Unset inherited `LF_*` and close stdin as described in `scripts/testing.md`.
`DesktopHeadlessTests/workStates` checks loading, unavailable, empty and selected
Work and taps the production navigation button. Report these as automated
checks, not a live-window walkthrough. Hosted window checks remain optional;
record what was not seen. With a display, `uv run python scripts/loopflow-dev.py
run-debug` builds and launches the development app; use `lf desktop state --json`
and the commands in `docs/surfaces.md` to inspect and drive it.

Use a private `LF_HOME` and an explicit branch `LF_BIN` for experiments. Clear
inherited `LF_*` first; isolate `HOME` and the repository too. Never attach the
Session running this QA step.

For headless relay checks, `lf resume --prepare --json` creates connection data
without launching a provider; repeating it keeps the same Session. Start the
branch's `lf server run` in the private Machine and parse `lf server status` as
JSON (its output is compact). Wait for `ready` before starting a relay, so its
initial readiness request does not register a background service. Stop and reap
the private server and every relay afterward.

A probe compiling `ServerConnection.swift` against real relay publications can
check mailbox delivery and cancellation without WindowServer. Label this as a
component probe: it does not establish mounted panes, hidden-pane rendering,
focus, copy/Find, or Move here interaction. Keep display diagnostics optional in
headless runs; record those limits and leave the full Swift suite and Xcode
fallback build to gate. See `swift/README.md` for the supported checks.
