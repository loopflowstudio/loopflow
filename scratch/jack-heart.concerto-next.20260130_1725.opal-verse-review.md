# Design Review: Improvise UX & Wave Creation Polish

## What was implemented

Three main improvements to Concerto's wave creation and improvise experience:

**1. Non-blocking wave creation**

Wave creation now returns immediately. The git operations (fetch, worktree add, push) run in a background task. The UI stays responsive while the worktree is being set up.

- `create_wave()` creates the database record and returns
- `setup_wave_worktree()` is called via `asyncio.to_thread()` in the HTTP handlers
- `wave.ready` event fires when worktree is available

**2. Prompt passthrough for interactive sessions**

Users can now type context in the StepRunner prompt field and have it passed to the agent. Previously the prompt field existed but was ignored.

- `InteractiveSession` has a new `prompt: String?` field
- `session.command` property builds the shell command with escaped prompt
- `shellEscape()` utility handles quotes and special characters correctly
- 6 unit tests verify shell escaping behavior

**3. Area typeahead with fish-style completion**

Replaced the static area display with an interactive typeahead:

- Tab completion shows ghost text like fish shell
- Multiple areas as chips with removal
- Excludes common noise directories (node_modules, __pycache__, etc.)
- Backspace removes the last chip when input is empty

## Key choices

**Background git operations.** Wave creation was blocking the event loop for 2-5 seconds (fetch + worktree + push). Moving git to a background thread lets the UI respond immediately. The wave appears in the sidebar right away; the worktree becomes available shortly after.

**Positional args for prompt.** The existing `step_args` machinery in `context.py` already appends positional args to step content. Using this path means no CLI changes needed—the prompt becomes part of the step instructions naturally.

**Fish-style ghost text over dropdown.** Path completion via ghost text is faster than a dropdown for terminal-native users. Tab accepts the completion, Enter commits the path. Matches the muscle memory of shell users.

**`longSession` for API calls.** Added a separate URLSession with 30s timeout for operations that hit the daemon while it runs git. The default 10s was causing timeouts during wave creation.

## How it fits together

```
Create Wave (click +)
    │
    ├── WaveService.createWave() → POST /waves
    │   └── Returns immediately with wave (no worktree yet)
    │
    ├── UI: Wave appears in sidebar, selected
    │
    └── Background: setup_wave_worktree()
        ├── git fetch origin
        ├── git worktree add -b <branch> <path> origin/main
        ├── git push -u origin <branch>
        └── Emit wave.ready event

Run Step (with prompt)
    │
    ├── StepRunner.runStep() passes prompt to SessionState
    │
    ├── InteractiveSession.command builds: "lf design 'add rate limiting'"
    │
    └── GhosttyTerminalView executes command
        └── Agent receives: step content + "\n\n" + prompt
```

## Risks and bottlenecks

**Background task failures are silent.** If worktree setup fails, the wave exists but has no worktree. The UI needs to handle this gracefully—currently it shows nothing when worktreePath is nil.

**Area completion is synchronous filesystem I/O.** `childrenOf()` reads the filesystem on each keystroke. For very large directories this could be slow, though the exclude patterns help.

**No cancellation for background worktree setup.** If user deletes a wave immediately after creating it, the background task continues. Not harmful, just wasteful.

## What's not included

- **Auto execution with prompt.** Flows run multiple steps autonomously; user prompt doesn't make sense mid-flow. The prompt field is for interactive steps only.

- **Wave.ready handling in UI.** The `wave.ready` event fires but nothing currently listens for it. Future work could refresh the wave's worktree state.

- **Area validation.** The typeahead doesn't verify paths exist before committing. Invalid paths will fail when the step runs.

## Test coverage

Swift tests: 70 tests pass, including 6 new tests for shell escaping and InteractiveSession.command behavior.

Python tests: 669 tests pass. Wave creation logic tested through existing wave/stimulus tests.

## Files changed

| File | Purpose |
|------|---------|
| `src/loopflow/lfd/wave.py` | Split create_wave and setup_wave_worktree |
| `src/loopflow/lfd/daemon/http_server.py` | Background worktree setup, asyncio.to_thread for git ops |
| `src/loopflow/lfd/daemon/server.py` | Better connection error handling |
| `swift/LoopflowCore/Models/Wave.swift` | Add prompt to InteractiveSession, shellEscape() |
| `swift/Concerto/State/SessionState.swift` | Pass prompt to launchInteractiveSession |
| `swift/Concerto/Views/Improvise/AreaTypeahead.swift` | New fish-style area input |
| `swift/Concerto/Views/Improvise/StepRunner.swift` | Use AreaTypeahead, pass prompt |
| `swift/Concerto/Views/InteractiveSessionView.swift` | Use session.command |
| `swift/LoopflowCore/Services/WaveService.swift` | longSession for git operations |
| `swift/ConcertoTests/WaveTests.swift` | Shell escape and command tests |
