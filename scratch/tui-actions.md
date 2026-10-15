# TUI copy and shortcut ownership — inherited evidence

Jack Heart requested the October 15 provider study. Complete source citations,
versions, observations, failed interpretations and artifact checks:
`d6743d844fbac89693e1e25ad3e9c2a98fe9fe02:scratch/tui-actions.md`.

Accepted: header-only terminal Copy confirmation; exact Cmd+C/V/F host shortcuts,
with other modifiers reaching Ghostty/provider bindings. Successful clipboard
writes confirm selection receipts even though Copy cell decoration is removed:
clear-on-copy must still work. Find retains its finite cue. First-responder
ownership prevents sibling panes consuming input.

Codex can copy through the native clipboard rather than OSC 52; its internal
“Copied” text does not establish terminal selection or a routing bug. Never infer
clipboard success by scanning output. Claude Ctrl+C interrupt/clear and Ctrl+V
image paste, and OpenCode's configurable copy/paste actions remain provider-owned.
Terminal Find sees only rendered terminal content. Shift-drag obeys Ghostty's
mouse-shift-capture plus application mode, not an unconditional host override.

Remaining demo evidence: owned disposable provider Sessions exercising drag,
terminal and provider copy, wrapped Unicode, paste, Ctrl+C, images, IME, Escape,
multiline input and remaps. No live provider input or clipboard mutation was
performed by the study. The parent artifact/check receipts remain in the archive;
LOO-488's combined build must retain these mechanisms.
