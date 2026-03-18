# Review: jack-heart.agent-embedding.20260317_1347

## What was implemented

Added persisted `terminal_sessions` support in `lfd` (types, SQLite/Postgres storage, HTTP routes, events, and wave-executor lifecycle hooks) so interactive wave steps can launch, resume, and resolve terminal-backed work by exit code.

On macOS, selected waves now open a workspace-first detail surface. `WaveWorkspaceView` keeps the native work/session view as the default tab and adds a Ghostty-backed Terminal tab only when that wave has an active terminal session. Supporting Swift state now tracks terminal sessions separately from chat/session state, and attention items decode backend kinds directly instead of collapsing them.

Updated reviewer-facing docs and wave notes to describe the shipped workspace model, including the correction in `swift/README.md` that terminal embedding is additive rather than a takeover.

## Key choices

- **Workspace first, terminal additive** — keep `WaveDetailPanel` as the default selected-wave surface and only show a Terminal tab when server state says one exists.
- **Server-owned terminal lifecycle** — store terminal sessions in `lfd`, not only in the client, so resume/failure behavior survives restarts and matches executor state.
- **1:1 attention-kind mapping** — decode backend attention variants directly in Swift instead of semantic collapsing, which keeps the UI honest and reduces translation code.
- **Separate terminal-session state** — `TerminalWorkspaceStore` owns terminal selection/order so terminal routing does not distort the existing interactive-session/chat path.

## How it fits together

`lfd` creates and updates terminal-session records while wave runs enter interactive steps, emits lifecycle events, and resolves the waiting run when the terminal exits. Swift fetches and subscribes to those session records through `LocalWaveService` and `LocalEventService`, stores them in `RepoState`/`TerminalWorkspaceStore`, and routes the selected wave through `ContentView -> WaveWorkspaceView -> (WaveDetailPanel | TerminalWorkspaceView)`.

The result is a single selected-wave workspace with optional terminal surfacing, instead of branching into a separate terminal takeover flow.

## Risks and bottlenecks

- **Ghostty dependency** — terminal embedding still assumes the Ghostty C library is available and linkable on reviewer machines.
- **Backend/store parity** — terminal-session CRUD now spans migrations plus both SQLite and Postgres implementations; schema drift would break resume behavior.
- **Local-only terminal transport** — remote repos still stay on the queue/detail path, so the workspace model is intentionally asymmetric for now.
- **UI automation environment** — package-level Swift tests pass, but the full Xcode UI-test invocation is sensitive to the local macOS UI environment.

## What's not included

- Remote terminal transport
- Multi-wave terminal grids or pane management
- Terminal layout persistence
- Wave settings/config redesign

## Validation

### Automated

| Suite | Result |
|-------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy -- -D warnings` | pass |
| `cargo test --all` | pass |
| `uv run pytest python/tests/` | 113 passed |
| `swift test --package-path swift` | 243 passed |
| `tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` | smoke pass + 16 passed |
| `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` | default DerivedData run failed locally with a stale-output write error |
| `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -derivedDataPath /tmp/LoopflowSwiftGate.<ts> CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` | app build + unit suites completed, then `ConcertoUITests-Runner` hung before establishing connection in this no-rendering environment |

### Manual product check

Not run here. This gate ran headless with no rendering environment, so the interactive Concerto flow still needs a logged-in macOS verification pass with:

```bash
uv run python scripts/concerto-dev.py run-debug
```

Verify:

1. Selecting a wave opens the work surface instead of a terminal takeover.
2. The Terminal tab appears only when the selected wave has a terminal session.
3. Exiting the terminal with status 0 resumes the wave; non-zero marks it failed.
4. No selection shows the repo-wide attention queue.
5. Attention items render with the backend kind they came from.
