# IDE attach: proven same-durable-session in VS Code and Cursor

## User-visible outcome

Open offers Claude in VS Code or Cursor as an **attach** target — not
worktree-only — when the provider is Claude, a provider session id is known,
and the workspace is proven on a local Home. The IDE opens at the worktree
and the integrated terminal runs the exact shared attach command, resuming
the same durable Session. A non-Claude provider or an unknown session id
keeps the IDE at worktree-only, labeled honestly. A remote Home or a missing
app stays unavailable.

## End-to-end proof

1. A Claude handoff with `provider_session_id = "sess_abc"` on a local Home
   with Cursor installed and the worktree present: `capability.reach(.cursor)`
   returns `.attach`. The launcher opens Cursor at the worktree, then runs
   the exact argv (`claude --resume sess_abc --cwd /src/repo`) in the
   integrated terminal via AppleScript. The preference records Cursor as the
   last successful attach surface for this provider on this Home.

2. A Codex handoff (provider != "claude") on the same machine:
   `capability.reach(.cursor)` returns `.worktreeOnly`. The launcher opens
   the worktree only. The preference is not overwritten.

3. A Claude handoff with `provider_session_id = nil`:
   `capability.reach(.cursor)` returns `.worktreeOnly`.

4. Remote Home: `capability.reach(.cursor)` returns `.unavailable`.

5. AppleScript failure (e.g. Accessibility permission denied): the launcher
   opens the IDE at the worktree but cannot run the command. The launch
   returns `.worktreeOnly` — the user sees the IDE open with an honest
   "Opened the worktree" message, and the preference is not recorded.

## Source of truth

The attach descriptor (`InteractiveHandoffAttach`) from `lf handoff attach
--json` is the authoritative command. The capability is a pure function of
the descriptor's host/cwd, the handoff row's provider and provider_session_id,
and the machine's installed apps. The launcher is the side effect.

## Affected surfaces

- `HandoffSurface.swift` — capability gains `providerIsClaude` and
  `providerSessionKnown`; `reach()` for IDEs becomes provider-aware.
- `HandoffSurfaceLauncher.swift` — new `launchIDEAttach` (AppleScript),
  three-way `HandoffLaunchResult`, `capability()` takes provider + session id.
- `ActiveSessionsView.swift` — `HandoffAttachSheet` passes provider info to
  capability, handles three-way launch result.
- `HandoffSurfaceTests.swift` / `HandoffSurfaceLauncherTests.swift` — new
  test cases for IDE attach, unsupported provider, unknown session, fallback.

## Absent and error states

- No provider session id: IDE is worktree-only (can't resume a specific session).
- Non-Claude provider: IDE is worktree-only (no IDE terminal attach path).
- Remote Home: IDE is unavailable (can't open a remote worktree locally).
- AppleScript failure: IDE opens at worktree, launch returns worktree-only.
- IDE not installed: unavailable.

## Exclusions

- Warp and Ghostty behavior is unchanged (already proven attach).
- The `--ide` flag is not added to the argv — the exact shared attach command
  runs as-is. IDE context comes from the IDE being open, not from a flag.
- No workspace file injection (no .vscode/tasks.json or settings.json).
