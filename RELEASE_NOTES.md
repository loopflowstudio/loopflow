# v0.12.11

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.11 makes local Ask review feel native in Ghostty without changing the session model underneath. On macOS, `lf ask open` now gathers Asks into one dedicated Ghostty window, reuses named tabs, and focuses the requested session instead of presenting a tmux-linked Ask hub. Other terminals and remote presentation paths keep their existing behavior.

## Review local Asks in one Ghostty window

Ghostty now owns the presentation layer while tmux continues to preserve each Ask as a distinct session. Reopening or concurrently presenting Asks therefore converges on the existing window and tabs rather than creating duplicate review surfaces.

- Set `LF_EXTERNAL_TERMINAL=Ghostty` and run `lf ask open ask_...` to open or reattach an Ask in its named tab.
- Loopflow remembers a dedicated Ghostty Ask window and focuses the requested tab when it already exists.
- Concurrent presentations are serialized so they do not create duplicate windows or tabs.
- Reattaching an existing Ask removes launcher scripts that are no longer needed.
- Each Ask retains its own tmux session; only its local macOS presentation changes.

## Operational notes

- Native Ghostty tab control depends on macOS Automation permission. If access is denied, `lf ask open` reports guidance for enabling it.
- The new routing applies only when Ghostty is selected as the external terminal. Other terminal integrations and remote Ask presentation are unchanged.
