# Pane routing spec

Date: March 19, 2026

## Goal

Keep the workspace feeling tmux-like even if Concerto owns the outer pane tree and tmux owns shell subdivision inside the single terminal pane.

The user should learn one pane-management vocabulary.

## Core model

Per wave:

- one recursive **outer** pane tree owned by Concerto
- exactly one `.terminal` outer pane
- that terminal pane hosts one Ghostty surface
- that Ghostty surface attaches to one tmux session
- tmux owns all **inner** shell splits/windows inside the terminal pane

## Rule: one command set

Pane-management shortcuts are configured once, as semantic commands.

They do **not** split into “outer shortcuts” and “terminal shortcuts.”

Good:

- `split_vertical`
- `split_horizontal`
- `close_focus`
- `focus_left`
- `focus_right`
- `focus_up`
- `focus_down`
- `resize_left`
- `resize_right`
- `resize_up`
- `resize_down`
- `zoom_focus`
- `new_tab`

Bad:

- `outer_split_vertical`
- `tmux_split_vertical`
- separate keymaps for native panes vs terminal panes

## Rule: same shortcut, focused-layer dispatch

A shortcut means “do this to the focused thing.”

Dispatch depends on focus:

- if focus is on a non-terminal outer pane, Concerto handles the command in the outer tree
- if focus is inside the terminal pane, Concerto translates the same semantic command into tmux actions

Examples:

- `split_vertical`
  - non-terminal focused → split outer pane vertically
  - terminal focused → tmux vertical split
- `close_focus`
  - non-terminal focused → close outer pane
  - terminal focused → kill/close tmux pane or tmux window, depending on terminal state
- `focus_left`
  - non-terminal focused → move outer focus left
  - terminal focused → move tmux focus left

## Focus model

There are two nested focus layers:

1. **Outer focus** — which native pane is active in Concerto
2. **Inner terminal focus** — when the terminal pane is outer-focused, tmux has its own active pane/window

### Routing rule

- If outer focus is on a non-terminal pane, pane commands go to Concerto.
- If outer focus is on the terminal pane and the terminal has keyboard focus, pane commands go to tmux.

### Visual requirement

The UI must clearly show:

- which outer pane is focused
- when the terminal pane is the focused outer pane

Without this, shared shortcuts will feel random.

## Outer pane invariants

- The outer layout is always a recursive split tree.
- Exactly one terminal pane exists per wave.
- The terminal pane cannot be duplicated.
- Closing the terminal pane is either disallowed or immediately replaced with a terminal pane in the resulting layout.
- Non-terminal panes are first-class leaves in the outer tree.

Suggested outer pane types for phase 1:

- terminal
- markdown
- diff
- launchpad

## Terminal invariants

- One Ghostty surface per wave.
- One tmux session per wave.
- Concerto does not attempt to map outer leaves to tmux windows or tmux panes.
- tmux is the only owner of shell splitting/tabbing inside the terminal pane.

## Command translation

Concerto owns the semantic command vocabulary.

When terminal is focused, it translates commands to tmux operations.

Example mapping:

- `split_vertical` → `tmux split-window -h`
- `split_horizontal` → `tmux split-window -v`
- `focus_left` → `tmux select-pane -L`
- `focus_right` → `tmux select-pane -R`
- `focus_up` → `tmux select-pane -U`
- `focus_down` → `tmux select-pane -D`
- `resize_left` → `tmux resize-pane -L <step>`
- `resize_right` → `tmux resize-pane -R <step>`
- `resize_up` → `tmux resize-pane -U <step>`
- `resize_down` → `tmux resize-pane -D <step>`
- `zoom_focus` → `tmux resize-pane -Z`
- `new_tab` → `tmux new-window`
- `close_focus` → `tmux kill-pane` or `tmux kill-window`, depending on active terminal topology

## Config model

Keybindings should be stored against semantic commands, not terminal-native strings.

Sketch:

```swift
enum PaneCommand: String, Codable {
    case splitVertical
    case splitHorizontal
    case closeFocus
    case focusLeft
    case focusRight
    case focusUp
    case focusDown
    case resizeLeft
    case resizeRight
    case resizeUp
    case resizeDown
    case zoomFocus
    case newTab
}

struct Keybinding: Codable {
    let command: PaneCommand
    let shortcut: Shortcut
}
```

Dispatcher shape:

```swift
enum FocusContext {
    case outerPane(paneId: String, kind: PaneType)
    case terminal
}
```

Routing:

```swift
func dispatch(_ command: PaneCommand, focus: FocusContext) {
    switch focus {
    case .outerPane(_, let kind) where kind != .terminal:
        outerPaneManager.handle(command)
    case .terminal:
        tmuxBridge.handle(command)
    default:
        outerPaneManager.handle(command)
    }
}
```

## Product behavior

This gives one consistent mental model:

- “split” always means split the focused thing
- “close” always means close the focused thing
- “move focus” always means move to the adjacent thing in the focused layer
- “resize” always means resize the focused boundary in the focused layer

The user does not need separate shortcut vocabularies for native panes and terminal panes.

## What this architecture is

This is **not** a fully native terminal multiplexer.

It is:

- a tmux-like native outer compositor
- with one embedded terminal workspace pane
- where the terminal workspace itself is powered by tmux

That is a much smaller and safer first milestone than native multi-terminal pane composition.

## Recommended next design change

Update the wave multiplexer design doc to say:

- one terminal pane per wave
- terminal pane backed by one tmux session
- Concerto owns outer recursive layout
- tmux owns inner shell layout
- one semantic shortcut map configures both layers
