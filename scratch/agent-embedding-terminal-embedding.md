# Wave Workspace: Ghostty + tmux as Infrastructure

## What this wants to be

Concerto should become the place where a conductor lives while managing multiple waves.

A wave is not just a row in a sidebar and not just a terminal tab. A wave is a workspace. The shell space for that workspace should feel tmux-native. The rest of the workspace should be native Concerto tools: file viewing, diff summarization, queue/context, GUI harnesses, Cursor handoff, PM handoff, native coding agents, and whatever other Swift surfaces belong around the work.

The core idea is no longer “embed a terminal and polish it.” The core idea is:

- **Ghostty** is the terminal rendering pillar
- **tmux** is the shell/session/layout/navigation pillar
- **lfd** is the loopflow control plane and remote boundary
- **Concerto** is the tmux-fluent native client that composes shell panes and Swift panes into one wave workspace

## Problem

The current branch proves that embedded terminal state can exist in the product, but it is still shaped like a custom terminal feature.

That is too small.

The real target is a workspace architecture with the right substrate:
- no coding on main
- every coding shell belongs to a wave worktree
- shell behavior uses tmux’s language and operational model where possible
- terminal rendering stands on Ghostty rather than a bespoke terminal stack
- Concerto owns the non-shell parts of the workspace without forcing them through terminal abstractions
- remote still has a clean API boundary through `lfd`

If Ghostty and tmux are the pillars, the design question changes from “what terminal UX should we add?” to “what architecture naturally grows around those pillars?”

## Architectural model

### Pillars

#### Ghostty

Ghostty is infrastructure, not the product model.

It owns:
- terminal rendering
- VT behavior
- embedded terminal surfaces
- local terminal display concerns

Concerto should not invent a second terminal rendering model above it.

#### tmux

tmux is also infrastructure, but it is much closer to the product model than Ghostty.

It owns the shell part of the system:
- windows
- panes
- attach / detach
- shell-session identity
- layout vocabulary
- navigation vocabulary
- resize and multi-client shell semantics

Loopflow should seriously use tmux, not merely imitate it.

#### lfd

`lfd` stays essential.

It owns:
- remote API boundary
- auth
- persistence
- wave / run / attention semantics
- loopflow metadata around shells
- tmux-aware proxy behavior

For remote, the authoritative shell substrate lives server-side next to `lfd`.

#### Concerto

Concerto is the client that brings it together.

It should:
- behave like a tmux client where shells are involved
- directly speak tmux-shaped operations where that makes sense
- compose tmux-backed shell panes with Swift-owned panes
- provide the native loopflow workspace around shell activity

## Product mapping

### One wave = one tmux window

This is the primary visible mapping.

A wave should feel like a tmux window with loopflow semantics attached to it.

Inside that wave window, the workspace can contain:
- tmux-backed shell panes
- open in Cursor
- open in PM
- file viewer
- diff summarizer
- native coding agent surfaces
- queue/context views
- other Swift-owned panes

### One region for shell panes and Swift panes

Do not split the UI into a tmux island plus a separate native app region.

There should be one composed workspace region. Some panes are shell panes. Some panes are Swift panes. The user experiences one workspace.

### TerminalSession is a thin wrapper

For shell panes, the tmux pane identity is primary.

`TerminalSession` should be a thin loopflow wrapper around that tmux pane identity, carrying things tmux does not know:
- wave binding
- run / step association
- agent metadata
- loopflow attention and resume semantics

It should not become a second shell runtime model.

## Ownership boundaries

### tmux owns

- shell panes
- pane/window identity for shell runtime
- attach / detach semantics
- pane lifecycle for shells
- shell navigation and layout primitives

### Ghostty owns

- terminal rendering of shell panes

### loopflow / Concerto own

- any pane that is not a shell and is rendered in Swift
- wave semantics
- attention semantics
- run semantics
- persistence and product meaning layered around tmux state

## Implications for the architecture

### 1. Stop deepening the custom local shim

The current launch-spec / callback / local-session machinery may be good enough as a bridge, but it should not become the center of the architecture.

If tmux is the shell substrate, new work should move toward:
- tmux-backed shell sessions
- tmux-native window/pane identity
- Ghostty-rendered shell panes
- `lfd` proxying and enriching tmux state

Not toward an ever-richer custom terminal session stack inside Swift and Rust.

### 2. Keyboard design should be tmux-fluent

Do not design shortcuts as a separate macOS command palette system with a terminal feature attached.

Do not preserve existing shortcuts just because they already exist.

Design one keyboard/navigation system that fits the surrounding ecosystem:
- tmux-like navigation and layout habits
- searchable navigation when needed
- loopflow actions integrated into that same system

The right test is not “does this look like a normal Mac menu shortcut set?” The right test is “does this feel natural to someone living in terminal/workspace tooling?”

### 3. Wave-homed shells only

Coding happens in wave worktrees.

No main-checkout shells. No repo-scoped freeform coding shells in this milestone.

If a shell is created from Concerto, it belongs to a selected wave and opens in that wave’s workspace.

### 4. Remote should still speak through lfd

Even if Concerto behaves as a tmux client, remote product semantics still go through `lfd`.

That means:
- authoritative remote tmux lives server-side
- `lfd` proxies or mediates the relevant tmux operations
- Concerto can speak tmux directly where appropriate, but the remote contract still belongs to loopflow via `lfd`

### 5. Swift panes should not be terminal-shaped

Native panes are not fake panes rendered through shell output.

File viewers, diff summarizers, queue/context panes, GUI harnesses, and native coding agents should remain Swift components. The composition model should unify them spatially with shell panes, not degrade them into terminal content.

## This milestone

This milestone should reshape terminal embedding around the new center rather than finishing the old design literally as written.

### In scope

- reframe the workspace architecture around Ghostty + tmux
- make wave-homed shell creation explicit
- make one wave → one tmux window the guiding mapping
- define `TerminalSession` as a thin wrapper around tmux pane identity
- make Concerto’s shell-side interaction model tmux-fluent
- keep `lfd` as the tmux-aware loopflow boundary, especially for remote
- define how shell panes and Swift panes coexist in one workspace region
- identify which current local-session mechanisms are bridge code vs target architecture

### Out of scope

- forcing every pane to be a shell
- repo-scoped or main-checkout coding shells
- treating existing Ghostty/session code as the final architecture
- a fully finished remote transport
- full composition implementation for every pane type
- solving all layout persistence details in this item

## Open design questions

### tmux integration depth

How much should Concerto speak tmux directly versus going through `lfd` for convenience APIs?

Current direction:
- `lfd` as tmux proxy and loopflow control plane
- Concerto speaking tmux directly where it cleanly can

### Window and pane mapping beyond the first shell

A wave maps to a tmux window.

What exact pane model should that window expose when there are:
- multiple agent shells
- utility shells
- Swift panes that need equal spatial status

### Composition model

The workspace should be one region for shell panes and Swift panes.

What is the cleanest layout engine for that mixed world while preserving tmux fluency where shell panes are involved?

### Transition plan

What bridge architecture gets us from the current Ghostty-embedded custom session model to a Ghostty + tmux-centered architecture without thrashing the whole branch?

## Done when

The design is ready to drive implementation of a tmux-centered wave workspace.

Concretely, that means the implementation plan should preserve these truths:
- a wave is a workspace and maps to a tmux window
- Ghostty and tmux are treated as infrastructure pillars, not incidental dependencies
- shell panes are tmux-backed
- Swift panes stay native
- `TerminalSession` is a thin wrapper, not a parallel shell model
- `lfd` remains the loopflow API/control boundary for remote
- the user experiences one coherent workspace, not separate terminal and native worlds
