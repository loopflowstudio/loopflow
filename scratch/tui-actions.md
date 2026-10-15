# TUI copy and shortcut ownership

Jack Heart requested a focused study of copy and action routing inside Codex,
Claude Code and OpenCode on October 15, 2026. Shell copying worked in the demo,
but the cell underlining was too much. The accepted refinement is header-only
copy confirmation; Find keeps its existing cue.

## Implemented refinement

The Ghostty patch no longer paints copy edges or schedules copy animation.
Selection receipts remain: successful clipboard writes still honor
`selection-clear-on-copy`, and a changed selection cannot be cleared by an old
receipt. Find keeps its finite renderer timer. The shader/color policy is
unchanged. This requires artifact lf7; dropping the host's confirm call would
have silently broken clear-on-copy.

Host Copy, Paste and Find now match exact Cmd+C/V/F. Previously an extra Control,
Option or Shift could still be intercepted as the ordinary host action. Those
combinations now reach Ghostty's bindings and negotiated terminal keyboard
protocol. Caps Lock does not change the host shortcuts. The existing
first-responder rule still prevents sibling panes from consuming the event.

## What the copy paths establish

`GhosttyMetalView` sends mouse events to Ghostty, which owns terminal selection
and application mouse reporting. On exact Cmd+C the host reads a Ghostty text or
shell-block selection, writes the clipboard, then confirms the receipt and shows
the pane-header notice. With no Ghostty selection, it returns false; ordinary
AppKit/Ghostty input handling remains available. It does not synthesize Ctrl+C.

A TUI can own its own selection and clipboard operation. The observed word
“Copied” inside Codex is consistent with that path, not evidence that Loopflow
stole the shortcut. Codex can write the native clipboard directly, so adding a
toast to Ghostty's OSC 52 callback would not unify all successful copies. Do not
infer success or mirror notifications by scanning terminal output.

Shift-drag is not an unconditional override. The pinned Ghostty source's
`mouseShiftCapture` considers `mouse-shift-capture` and the application's mode;
`never` and `always` override it, while ordinary defaults can be superseded by
the application. Keep those semantics instead of globally disabling TUI mouse
actions. The earlier instruction that Shift-drag necessarily forces selection
was too strong.

## Provider evidence

Installed versions were read without starting a Session: Codex 0.162.1,
Claude Code 2.1.300, OpenCode 1.18.33.

| Provider | Verified ownership and implications |
| --- | --- |
| Codex | Its versioned source has transcript/composer selection and asynchronous clipboard completion. Local copying prefers the native clipboard; OSC 52 is a fallback, or terminal forwarding in SSH/tmux. Its keymap gives “copy last response” Ctrl+O by default. Preserve that action and negotiated keyboard reporting. |
| Claude Code | Official docs distinguish Ctrl+C interrupt/clear from Ctrl+V image paste. Fullscreen transcript view can export to native scrollback for terminal Find; terminal Find alone cannot search text the TUI has not rendered. Keep provider controls available. |
| OpenCode | Official docs define configurable actions, including `<leader>y` for message copy and Ctrl+V for prompt paste. Defaults and remaps belong to OpenCode, so do not translate host Copy into one hardcoded provider command. |

Sources: [Codex clipboard implementation at the installed tag](https://github.com/openai/codex/blob/rust-v0.162.1/codex-rs/tui/src/clipboard_copy.rs),
[Codex selection ownership](https://github.com/openai/codex/blob/rust-v0.162.1/codex-rs/tui/src/transcript_view/selection.rs),
[Codex keymap](https://github.com/openai/codex/blob/rust-v0.162.1/codex-rs/tui/src/keymap.rs),
[Claude Code interactive controls](https://code.claude.com/docs/en/interactive-mode),
[OpenCode keybinds](https://dev.opencode.ai/docs/keybinds).
Downloaded Codex source evidence is retained under
`.lf/tmp/terminal-feedback/tui-source/`; Ghostty source is under
`swift/.build/ghostty-kit/ghostty-a60e9e2a57f73e1eef2bd1cf2995a467f69e7fb0/`.

## Remaining demo evidence

No live provider was started, taken over, or sent keystrokes, and no test changed
the system clipboard. Compare plain drag, configured terminal-selection drag,
Cmd+C, provider-native copy, paste, and Ctrl+C in an explicitly owned disposable
Session for each provider. Verify pasted text, especially wrapped Unicode, and
which layer displayed the notice. Keep native-copy and terminal-copy success
distinct; image paste, IME, Escape, multiline input, and provider remaps need to
remain usable. This review has not established those live outcomes.

Check: `uv run python scripts/loopflow-dev.py ghostty-build` PASS with Zig 0.16.0;
focused Zig suites, production Metal 324 cases (168 unattainable ratios classified),
GLSL compilation, universal framework; `.lf/tmp/terminal-feedback/lf7-build.log`.
lf7 immutable upload and public-download SHA-256 match PASS:
`32e1d35ca909f67191e0d85afba8b52eb93f95a37513efbbb75f2caf195b44ed`.
Swift shortcut regression joins the caller's combined validation against lf7.
