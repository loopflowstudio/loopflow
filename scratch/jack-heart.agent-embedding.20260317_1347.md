# PTY-backed interactive runs in arbitrary paused waves

## Goal

Make a paused wave runnable from Concerto in the same way a human would run it in a terminal:

- pick any paused wave
- type `design` (or another step/flow) in the header typeahead
- click **Run**
- get the normal interactive `lf` experience inside the embedded terminal
- have that process run inside the environment `lfd` chose for the wave

The demo target is simple: `design` works from any paused wave, not just a special local-only path.

## Current problem

The current terminal-session path is the wrong abstraction for interactive steps.

- Concerto launches a terminal session from `TerminalLaunchSpec`
- `lfd` currently stores a prepared agent command in `TerminalSession.argv`
- the embedded terminal runs that raw agent command directly
- this is not the same as running `lf design`
- it also does not scale to non-local executors

That creates two visible failures:

1. the UI says "run `design`" but the terminal path is not the normal CLI experience
2. interactive runs only really make sense when the app can locally launch the command

## Product decision

Keep `lfd` as the control plane and auth/runtime boundary.

Do not make Concerto locally fake daemon execution for interactive steps.

Instead:

- `lfd` owns interactive terminal sessions
- interactive terminal sessions are real PTYs
- the command inside the PTY is the normal loopflow CLI entrypoint (`lf <step-or-flow>`)
- Concerto and future `lfq` clients attach to that PTY

This preserves:

- local auth and remote auth in `lfd`
- local/container/sandbox/remote executor choice in `lfd`
- one interactive model that works beyond local development

## Desired UX

For a paused wave in Concerto:

1. select the wave
2. header shows the wave title, a typeahead, and a big **Run** button
3. choose `design`
4. click **Run**
5. wave briefly transitions into a starting/running state
6. a terminal tab appears automatically
7. the terminal shows the normal `lf design` interactive experience
8. when the session exits:
   - exit `0` resumes/completes the wave run
   - non-zero exit fails the run

This should work for any paused wave, regardless of whether the executor is local or containerized.

## Architecture

### 1. Terminal sessions become PTY sessions, not launch specs

Today `TerminalSession` is effectively "metadata + a locally launchable command".

Replace that mental model with:

- terminal session id
- wave id / wave run id
- executor target
- requested command (`lf design`, `lf review`, `lf build`, etc.)
- PTY lifecycle state
- attach token / completion semantics

The PTY lives on the `lfd` side.

Concerto attaches to the PTY. It does not become the process owner.

### 2. Interactive runs use `lf`, not raw agent argv

For interactive steps/flows, `lfd` should run the standard CLI command in the executor environment.

Examples:

- `design` -> `lf design`
- `review` -> `lf review`
- `ship-roadmap` -> `lf ship-roadmap`

The requested flow/step override chosen in Concerto becomes the command passed to the PTY session.

The important rule: the command the human sees in the terminal should match the loopflow command they would run by hand.

### 3. `lfd` owns the executor environment

`lfd` must be the layer that decides where the PTY process runs:

- local worktree
- docker worktree/container
- sandbox worktree
- future remote executor

Concerto should not need executor-specific logic.

### 4. PTY transport plus structured state

Use two channels:

- **PTY transport** for live terminal interaction
  - stdin bytes
  - stdout/stderr bytes
  - resize
  - signals / close
- **structured events** for wave and terminal lifecycle
  - wave started
  - wave waiting
  - terminal session created
  - terminal session attached
  - terminal session exited
  - run resumed / failed

Do not try to make event logs impersonate a terminal.

## Backend changes

### `lfd`

Add a daemon-owned PTY/session manager.

Responsibilities:

- create PTY-backed interactive sessions
- start `lf <step-or-flow>` in the wave executor environment
- track session status
- expose attach/read/write/resize/close APIs
- emit structured events when session state changes
- reconnect to running sessions after UI detach if possible

### terminal session API

The current attach flow returns a `TerminalLaunchSpec`.

Replace or extend it with an attach protocol for server-owned terminals:

- create interactive run
- attach to session
- send input
- receive output stream
- resize terminal
- stop/cancel terminal

HTTP + websocket stream is fine. The important part is that the server owns the PTY.

### wave run integration

For interactive flow actions:

- create the wave run
- create the PTY terminal session for the chosen flow/step
- mark the wave as waiting on that terminal session
- when the PTY process exits:
  - success -> advance run
  - failure -> fail run

Interactive runs should not require a separate "launch raw agent command" codepath.

## Concerto changes

### header run bar

Keep the current direction:

- big run button in the main header
- typeahead next to it
- paused waves behave like runnable waves

But wire it to "start daemon-backed PTY run" rather than "launch a local command spec".

### terminal tab behavior

When a wave gets a live PTY session:

- automatically surface the terminal tab
- auto-select it for interactive runs launched from the header
- keep Work as the default for passive inspection

### session rendering

Ghostty should render the attached PTY stream.

Ghostty should not be responsible for process launch details beyond attaching to the PTY stream/surface.

## CLI follow-on

After the PTY primitive exists, `lfq` can grow an attached-interactive command such as:

```bash
lfq run <wave> -- design
```

That should use the same daemon-owned terminal session primitive as Concerto.

This is a follow-on benefit, not a prerequisite for the UI demo.

## Scope for this build

Build the smallest complete slice that proves the model:

1. paused wave header run bar can launch an arbitrary step/flow override
2. `design` starts a daemon-owned PTY-backed session
3. Concerto attaches the embedded terminal to that PTY
4. exit status drives wave/run completion correctly
5. local executor works end-to-end

If container execution falls out naturally from the PTY abstraction, take it.
If not, leave clear seams so the same API can support it next.

## Explicitly out of scope for this pass

- multi-client collaborative input
- perfect reconnect with preserved scrollback across app restarts
- replacing all non-interactive execution with PTYs
- polishing every terminal UX edge case

## Risks

### PTY transport is harder than event replay

Need correct handling for:

- raw bytes
- resize
- close semantics
- orphan cleanup

### scrollback and reconnect

A terminal emulator's local scrollback is not enough for detach/reattach.

For this pass, prioritize a live attached session. Durable replay can come later.

### executor divergence

The PTY abstraction must not assume local-only process launch.

Avoid baking host-local shell assumptions into the attach protocol.

## Done when

### Manual

Run:

```bash
uv run python scripts/concerto-dev.py run-debug
```

Verify:

1. Bootstrap still creates/attaches paused roadmap waves.
2. Selecting a paused wave shows the header run bar in the main workspace.
3. Changing the header typeahead from the default flow to `design` and clicking **Run** starts `design`, not the default flow.
4. The wave enters an interactive waiting state and automatically surfaces a terminal tab.
5. The terminal experience is the normal interactive `lf design` experience, not a raw agent subprocess launched directly by Concerto.
6. Exit `0` resumes/completes the run.
7. Exit non-zero fails the run.
8. Running the same path from another paused wave works without special-casing the wave.

### Automated

Run focused tests for:

```bash
swift test --package-path swift
cargo test --all
```

At minimum, add coverage for:

- manual run override on paused waves
- interactive run creating a PTY-backed terminal session
- terminal session exit status resuming/failing the wave run
- Concerto reacting to interactive waiting events by surfacing the terminal session
