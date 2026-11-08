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

## Fix and repeat

Repair what breaks within the change's scope, then go through the experience
again from the start. Keep going until it works for the people it is for. A
change to what the product should be is not yours to make; describe it
instead. When asked for a read-only review, report instead of
fixing.

Run tests only where they tell you something the real path cannot reach, and
keep any you add about what a person would notice.

## Keep what you learned

How to launch and drive this product belongs in this skill. When you had to work
out a step it does not describe, or a step it describes no longer works, write
the working version here with `lf skill customize qa`, replacing what was wrong.

Show what you did in `scratch/qa.html`: each scenario, what you did, what you saw
with screenshots or transcripts, and what you fixed. Finish with what works now,
what you could not reach and why, and what is still wrong, worst first. Do not
publish, land or choose the next Graph step from here.

## Terminal and server QA

Build `cargo build -p loopflow --bin lf`, then drive that executable. Use a
temporary Git repository and an isolated `LF_HOME`; clear inherited `LF_*`
before setting that home and `LF_BIN`. Clear terminal-host placement variables
when creating a separate PTY. Never run a QA rollover against the working Machine.

`lf session serve [--terminal] -- /bin/sh -c '…'` starts a real owner and prints
its Process ID. Its socket is under `/tmp/lf-<uid>-<sha256(canonical LF_HOME)[:16]>/`.
Use the protocol in `rust/loopflow/src/transport/README.md` to attach independent
clients. `LF_VIEWER_STREAM=1 lf session attach OWNER` exercises the raw relay;
ordinary `lf resume SESSION` belongs on a PTY, since its startup can paint a
saved screen before attachment. Do not treat those startup bytes as framed output.

Run `lf server run` with the same home and wait for a successful handshake, not
just the socket file. Use `hello`, `session.attach`, then the returned ticket on
a second connection as documented in `rust/loopflow/src/server/README.md`.
`LF_SERVER=1 LF_SERVER_STATUS=/tmp/PRIVATE/status.json lf session attach OWNER
--session SESSION` reaches Desktop's decoded relay and status mailbox. An initial
free-input attachment can refuse while another viewer controls the Session;
use an explicit takeover when that is the scenario.

Create provider history through a real interactive `lf -i -a codex : MESSAGE`
on a PTY, using configured authentication. A minimal no-tools reply is enough.
Exercise replacement with `lf resume` or `lf process rollover`, retaining the
same Session ID; a manually rebound database row does not establish that path.
For native slash commands, type the text, let completion render, then press
Enter. Check the resulting screen rather than assuming a pasted command ran.

The shell runtime is headless: in the sandbox, `lf wt create NAME` and
`lf task create --worktree PATH --title TITLE` provide a real Task checkout
without starting a Graph. Use `lf desktop shell TASK --checkout PATH --id ID
--create`, then the same command with `--close`. Inspect both the final frame
and background-job cleanup. Stop every owner, relay and server started by QA.
Mailbox checks establish relay behavior; mounted Desktop presentation still
belongs to a display-capable check.
