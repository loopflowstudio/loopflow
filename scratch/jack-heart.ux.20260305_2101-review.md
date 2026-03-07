# Review: Provider auth detection & eager daemon startup

## What was implemented

Local credential detection for Claude and Codex providers, eager bundled daemon startup, and several UX improvements to the connection flow.

**Rust (provider_auth.rs):**
- Codex token extraction now reads nested `tokens.access_token` from ChatGPT-style `auth.json` files, matching the actual Codex CLI auth format.

**Swift — Credential detection (CredentialSocketServer.swift):**
- `FileCredentialReader` reads Claude credentials from `~/.claude/.credentials.json` and Codex credentials from `~/.codex/auth.json`, including nested ChatGPT token format.
- Socket server serves these via `/credentials/{provider}` endpoints for the containerized daemon.
- Keychain fallback for providers that store tokens there (GitHub via `gh`, Claude/Codex safe storage).

**Swift — Eager daemon startup (BundledDaemonManager, RepoState, ConcertoApp):**
- `SharedDaemon.eagerStart()` fires at app launch, so the bundled daemon is already starting before the user opens any repo window.
- `RepoState.start()` joins an in-flight eager start via `startTask` deduplication.
- New `hasCompletedInitialLoad` flag and `isActivelyConnecting` computed property give the sidebar accurate connecting/loading state.

**Swift — Connection handshake optimization (RepoState):**
- Bundled daemon path skips TLS trust check and repo discovery (unnecessary for localhost).
- `connectLfd` immediately sets `.connecting(.startingDaemon)` phase for instant UI feedback.

**Swift — Sidebar cleanup (WaveSidebar, WorktreeRow):**
- Removed orphan worktree section and `WorktreeRow` view entirely.
- Added `connectingState` view showing progress spinner during daemon startup.

**Scripts (concerto-dev.py):**
- `_stop_concerto_app` handles graceful quit with forced fallback when modal dialogs block `osascript quit`.

## Key choices

1. **Eager start with task deduplication** — `eagerStart()` creates a `Task` stored in `startTask`; later `start()` calls join it via `await task.value`. Simple, no race conditions.

2. **Skip TLS/repo discovery for bundled mode** — The bundled daemon is localhost with a generated token. TLS trust check and repo listing are unnecessary overhead that adds visible latency.

3. **File-based credential reading over CLI shelling** — Reading `~/.claude/.credentials.json` and `~/.codex/auth.json` directly is faster and more reliable than shelling out to `claude auth status` or `codex auth status`, which may not be installed.

4. **Remove orphan worktree sidebar section** — This was a low-value feature that added complexity. Worktrees are managed via `lf ops wt` in the terminal.

## How it fits together

App launches → `SharedDaemon.eagerStart()` → daemon starts in background → user opens repo window → `RepoState` joins the in-flight start task → sidebar shows "Starting daemon..." → daemon healthy → event subscription starts → waves load → sidebar shows waves.

The credential socket server runs alongside the containerized daemon, serving local auth state (file + keychain) to lfd inside the container.

## Risks and bottlenecks

- **Eager start cost on launch** — If the user never opens a repo window, the daemon process is wasted. Low cost (single process), but worth noting.
- **File credential parsing is best-effort** — If Codex or Claude change their auth file format, detection silently fails (returns nil). The design doc acknowledges this and proposes terminal-assisted flows as fallback.
- **No cleanup of temp socket files on crash** — `stop()` removes the socket, but a hard crash leaves orphaned `.sock` files in `/tmp`. `start()` does `removeItem` before bind, so this self-heals on next launch.

## What's not included

- Provider-specific auth flows (device code, terminal-assisted login) — this is Phase 1 of the design doc, focused on detection and honest status display.
- Typed `AuthStep` model replacing the generic `AuthFlow` — planned for Phase 2.
- Provider provenance display in the UI — the data is available but card UI changes are deferred.
