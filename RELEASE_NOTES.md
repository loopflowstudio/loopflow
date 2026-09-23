# v0.12.19

v0.12.19 keeps ongoing terminal work intact as you navigate the Mac app. Session panes retain their running clients and contents, moving a Session from another terminal requires an explicit action, and completed shell commands can be copied together with their output. The through-line is continuity: preserve the work already running and make terminal actions follow your focus.

## Keep Sessions running as you move around

Closing a Session pane or switching views should not interrupt its client. The app now retains pane layouts and live terminal surfaces per repository within each window, so returning to a Session restores the ongoing work.

- Close and reopen Session panes, navigate between Sessions and Work, or switch repositories without losing the running client or terminal contents.
- Selecting a Session active elsewhere leaves its client running until you choose **Move here**. The previous terminal then reports that the Session moved.
- VIEWING, RUNNING, and ELSEWHERE states clarify terminal status, with recovery from provider exits.
- Opening a Session beside a focused shell preserves that shell.

## Copy a command and its output together

Shell panes now support selectable command blocks through the bundled Ghostty build. Completed commands and their output can be highlighted and copied as one block while the live prompt remains editable.

- Click a completed command block and press Command-C to copy the command and output.
- Terminal shortcuts follow the focused pane, including paste routing through split panes into live terminals.
- Clipboard, image, and file-drop handling broaden terminal input support.

## Operational notes

- Terminal retention lasts for the window's lifetime. Closing a shell pane still ends its shell.
- **Move here** discards unsent text in the previous client; the UI explains this before takeover.
- Command blocks apply only to shell panes with supported shell integration. The pinned build excludes automatic integration for macOS `/bin/bash`.
- The Ghostty framework is pinned with a checksum and bundled with matching shell integration and terminfo. A rebuild command is available for contributors maintaining the patched framework.

## Small changes

- Intentional provider termination for a Session move or completion exits cleanly; unexplained termination remains an error.
