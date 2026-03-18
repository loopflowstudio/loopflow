# LFD tmux architecture study

## Conclusion

tmux is the right reference for **session identity, detach/reattach, client/server split, and structured attachment protocol**. It is the wrong reference for **execution semantics**. Loopflow should not move flow execution into `lfd` just because tmux owns PTYs.

The research target is still ambitious: loopflow should become the best way to host SSH-style sessions into your coding agents. Local Ghostty plus shared-store observation is the staircase, not the destination. The destination is a runtime that can host long-lived agent sessions cleanly enough that SSH access feels first-class rather than bolted on.

The clean loopflow shape is:

- `lf` is the execution language
- a shared runtime store is the durable truth
- `lfd` is a host around that truth: scheduler, supervisor, fanout, later shell host
- Concerto and terminals are clients of the same runtime model

So the first staircase should be:

1. shared-store observation
2. local Ghostty embedding
3. automated runs via real `lf`
4. daemon-owned PTYs
5. remote access

That gets the tmux lessons we need without importing tmux's whole product model too early.

## What tmux teaches cleanly

From the tmux man page:

- a **session** is a persistent collection of PTYs managed by one server
- a **client** displays a session and can detach and reattach
- the **server** owns PTYs and survives client disconnects
- multiple clients may attach to the same live session
- tmux exposes a **text control protocol** with command output blocks and asynchronous notifications

Those are the ideas to borrow. The specific UI vocabulary of windows and panes is much less important.

## Mapping tmux concepts onto loopflow

| tmux concept | What it means in tmux | Loopflow equivalent | Copy / adapt / avoid |
|---|---|---|---|
| Server | Long-lived daemon that owns PTYs and session state | Shared runtime store first; `lfd` as supervisor/fanout host; later PTY host | **Adapt** |
| Client | Attached viewer/controller process | Concerto, terminal UI, mobile app, CLI helper | **Copy** |
| Session | Persistent identity that survives detach | `TerminalSession` once PTYs exist; before that, the closest durable identity is the observed run/session record in the store | **Copy** |
| Window | Top-level screen inside a session | No direct runtime equivalent; maybe a future workspace/composition concern | **Avoid for now** |
| Pane | Rectangular sub-PTY inside a window | No direct runtime equivalent; if it exists later, it belongs in compositor policy, not the execution contract | **Avoid for now** |
| Socket / control connection | Structured client/server transport | Shared-store writes first; later attach protocol for daemon PTYs | **Adapt** |
| Detach / reattach | Client comes and goes; session survives | Required later for daemon PTYs; not required for local Ghostty v0 | **Copy later** |

## What changes because local v0 has no daemon PTY

The biggest change from a naive tmux-inspired design: **Loopflow does not need a tmux-like server first.**

If `lf` can write structured lifecycle state into a shared runtime store, then:

- manual CLI runs are observable immediately
- Concerto can become useful before it hosts shells
- `lfd` can shrink toward supervision instead of carrying flow semantics
- local embedded terminals can just be ordinary Ghostty sessions

That means some tmux questions are deferred, not ignored.

### Not step zero

- server-owned PTY lifecycle
- full detach/reattach semantics
- terminal resize/read/write protocol
- scrollback persistence
- pane/window modeling

### Step zero

- durable run/session identity in the store
- structured lifecycle events from `lf`
- client ability to discover current work and foreground it
- agent-embedding UI built around session identity instead of launch commands

## Copy / adapt / avoid

### Copy

#### 1. Stable identity outlives attachment

tmux gets this right. A client is disposable; the session is not.

Loopflow should keep that principle:

- attachments are ephemeral
- session/run identity is durable
- mobile and desktop can both refer to the same live thing

#### 2. Multiple clients are part of the model

tmux assumes multiple clients can attach. Loopflow should too.

That does **not** mean rich multi-user UX on day one. It means the model should allow:

- one live session
- zero or more attachments
- possibly one active input owner at a time

The mobile + desktop case makes this real, not hypothetical.

#### 3. Structured control beats terminal scraping

tmux control mode matters because it separates:

- command responses
- asynchronous notifications

Loopflow should keep the same instinct. The runtime contract should be structured and eventful, not inferred from terminal text.

### Adapt

#### 4. The "server" is partly a store before it is a PTY host

In tmux, the server is immediately the owner of PTYs and sessions.

In loopflow, the first durable center should be the **shared runtime store**. `lfd` can sit around that store and add supervision, launches, fanout, and later PTY hosting.

That is the main adaptation.

#### 5. Session is narrower than tmux session

A tmux session is a whole live workspace with windows and panes.

Loopflow's durable object should stay smaller:

- `WaveRun` = execution unit
- `TerminalSession` = live interactive terminal identity when needed
- workspace layout = client/compositor concern

Do not overload one object to mean run + terminal + layout + attachment graph.

#### 6. History should be structured first, scrollback second

tmux can buffer and expose terminal output. Loopflow does not need full-fidelity terminal history as the product contract in v1.

The durable history that matters first is:

- command start/stop
- resolved flow/step
- wait points
- failures
- outputs worth structuring explicitly

That should come from `lf` writing into the store.

### Avoid

#### 7. Do not import windows/panes into the runtime contract

Windows and panes are tmux's UI/container model. Loopflow does not need them in the execution architecture.

If Concerto later grows split views, that should stay a client composition model unless a stronger reason appears.

#### 8. Do not make `lfd` the source of flow semantics

This is the easiest mistake to make when borrowing from tmux. tmux owns shell hosting, so it is tempting to let `lfd` own execution logic too.

That would lose the main elegance:

- running `lf build` directly
- running it from Concerto
- running it from automation

should all mean the same thing.

#### 9. Do not require daemon PTYs before local usefulness

If local Ghostty plus shared runtime state gets us the product value, take that path first.

PTY hosting should earn its complexity by solving a real next problem:

- reconnect
- remote access
- multi-client live attachment

## Recommended staircase

### V0 — shared-store observation

`lf` discovers the shared runtime store and writes structured lifecycle events if available.

Effects:

- manual CLI runs become observable
- no daemon execution engine required
- "bring your favorite TUI" becomes true

### V1 — local Ghostty embedding

Concerto uses the shared store to know what is active and opens ordinary local Ghostty sessions.

Effects:

- embedded local work without fake transport
- `TerminalSession` and workspace UI can harden around durable identity

### V2 — automated runs via real `lf`

`lfd` launches normal `lf <flow-or-step>` commands and supervises them against the same store.

Effects:

- manual and automated runs converge
- daemon executor logic shrinks

### V3 — daemon-owned PTYs

Now tmux's server/client lessons matter directly:

- live PTY ownership
- attach/read/write/resize
- reattach after disconnect
- multiple clients attached to one session

### V4 — remote access

Only here decide whether remote starts as:

- SSH into host/container
- or a custom daemon PTY transport

The shared store and CLI contract should stay unchanged either way.

This is also the point where the product goal sharpens: if loopflow wants to be the best way to host SSH sessions into coding agents, the remote model needs to feel better than "raw SSH plus vibes." The likely differentiators are durable agent/run identity, structured history, queue/calibration context, and clean reattachment across clients.

## Requirements for the SSH-agent-host future

If the long-term goal is "the best way to host SSH-style sessions into your coding agents," the runtime eventually needs more than generic terminal hosting.

### Required

#### 1. Durable identity above the shell

An SSH session alone is not enough. A human should be able to answer:

- which wave/run is this?
- which agent/session is this?
- what was it trying to do?
- what happened before I attached?

This means shell identity must sit under run/session identity, not replace it.

#### 2. Reattach across clients

Desktop and mobile should be able to find the same live session. The current controlling client may change; the session identity should not.

#### 3. Structured history around the shell

The best host is not just a PTY relay. It gives you durable context:

- start/stop
- resolved flow/step
- waits/failures
- human checkpoints
- related queue/calibration state

That is the layer raw SSH does not provide by itself.

#### 4. Clean "bring your favorite TUI" compatibility

The remote host should still respect CLI-native execution. Running `lf` inside an SSH session should participate in the same runtime model as app-launched or daemon-launched work.

#### 5. Input ownership without identity confusion

Multi-attach should not mean chaos. The model should distinguish:

- who is attached
- who is currently allowed to type
- what session they are all looking at

That can start simple, but it needs to exist.

### Nice later, not first

- rich shared cursor/presence UX
- durable full-fidelity terminal scrollback
- pane/window topology as part of the runtime contract
- collaborative editing semantics beyond turn-taking or takeover

### Anti-goal

Do not build a generic SSH terminal manager that happens to run coding agents. The value is that loopflow makes agent work legible, attributable, and steerable.

## Implications for agent embedding now

These are the first things agent-embedding can do that line up with the later `lfd` work instead of fighting it:

1. **Keep `TerminalSession` as the durable handle.**
   Queue actions, workspace routing, and foregrounding should all target session IDs.

2. **Stop deepening the launch-spec shim.**
   It is a bridge, not the contract.

3. **Keep local terminal embedding local.**
   Ordinary Ghostty sessions are good enough for the first product win.

4. **Build around observed state, not around who launched the command.**
   CLI-started, app-started, and later daemon-started work should look like one runtime.

5. **Keep compositor work out of the runtime model.**
   Tabs, splits, and window composition should compose over session identity, not redefine it.

## Open design questions for 02

These belong in the daemon-aware CLI contract next:

1. How does `lf` discover the shared runtime store locally?
2. What is the smallest event schema that can correlate:
   - manual CLI runs
   - app-launched runs
   - daemon-launched runs
3. Which existing `terminal_sessions` fields survive unchanged once the runtime is shared-store-first?
4. Does the first contract need explicit attachment records, or are run/session IDs enough until daemon PTYs exist?
5. What is the minimum auth/safety boundary for local store writes?

## Verdict

Borrow tmux's **session identity, detach/reattach philosophy, multi-client model, and structured control instinct**.

Do **not** borrow tmux's assumption that the server must be the first place execution becomes real.

For loopflow, execution becomes real in `lf`. The store makes it legible. `lfd` and Concerto build around that.

The long-term bar is not merely "embedded terminals work." It is "SSH-style access to coding agents feels native, supervised, and legible." This tmux study should be judged against that bar even while it recommends a smaller local-first staircase.
