
---

## Follow-up: make the embedded terminal feel terminal-native

**Symptom:** In Claude Code and other TUIs, ordinary typing sometimes feels like paste/injected text instead of direct terminal keystrokes. The app reports "pasting text" and input feels clunky.

**Current behavior:** `GhosttyMetalView.keyDown` still routes many normal printable keys through AppKit text input:

- `interpretKeyEvents([event])`
- `insertText`
- `ghostty_surface_text`

That path is correct for IME composition and actual paste, but it is the wrong default for ordinary terminal typing. It makes the terminal feel like a text field hosting a terminal surface instead of the terminal being the primary input surface.

**Desired behavior:**

- **Ordinary keypresses** → `ghostty_surface_key`
- **IME composition / committed composed text** → `ghostty_surface_text`
- **Actual paste** → `ghostty_surface_text`

In other words: real typing should go through key events first, with attached `key.text` for printable keys. `insertText` should be a narrow path for IME and paste-like input, not the default path for every printable character.

### Build target

Refactor `GhosttyMetalView` so focused terminal panes behave like a real terminal:

1. **Prefer `ghostty_surface_key` for normal typing**
   - Printable single-key input should bypass `interpretKeyEvents` unless IME composition is active
   - Attach `key.text` for printable keys so Ghostty receives character + key semantics together
   - Keep control/command chords on the key-event path

2. **Restrict `insertText` to IME / committed text**
   - Continue to support marked text and composition for Japanese/Korean/Chinese input
   - Continue to use `ghostty_surface_text` for committed composed strings
   - Do not use `insertText` as the primary path for ordinary Latin-character typing

3. **Keep paste explicitly paste**
   - `Cmd-V`, context-menu paste, and clipboard insertion should still call `ghostty_surface_text`
   - This keeps bracketed-paste / paste semantics separate from typing semantics

4. **Preserve app-level pane shortcuts only where intentional**
   - Pane-management shortcuts like `Cmd-\\` and `Cmd-Shift-Return` can still be intercepted by Concerto
   - Once inside the terminal, everything else should feel like "we are a terminal", not "we host a text view"

### Validation

Manual checks:

- Claude Code no longer reports ordinary typing as paste-like input
- typing in shell prompt feels immediate and character-by-character
- `Ctrl-b`, `Ctrl-c`, `Ctrl-l`, `Esc`, arrows still work
- `Cmd-V` still pastes
- IME composition still works

If this build target succeeds, the embedded terminal can stop feeling like a wrapper around a terminal and start feeling like the terminal surface itself.
