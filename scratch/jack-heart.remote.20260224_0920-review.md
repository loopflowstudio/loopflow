# Review: Remote Connection Step 0

## What was implemented

Three pieces folded into one branch:

1. **Error mapping** — `WaveServiceError.daemonTimeout` for HTTP 502/504 from Caddy, with `DisconnectReason.daemonTimeout` surfaced in Concerto's connection status. HTTP 502 maps to `.serverUnreachable`.

2. **Remote editor/terminal launch** — `RepoTarget.remote(path:host:)` now carries the connection host. `TerminalLauncher` gains `openTerminal` and `openInIDE` with `remoteHost` parameter that dispatches to SSH-based remote openers (Cursor/VSCode via `--remote ssh-remote+host`, Zed via `ssh://host/path`). `openTerminalRemote` opens an SSH session in the user's terminal. "Copy SSH Command" replaces "Reveal in Finder" when remote.

3. **ship-roadmap flow** — New flow `ingest → kickoff → review-design → ship → review` with the `review-design` interactive step. Default flow for new waves changed from `design` to `ship-roadmap`.

## Key choices

- **Host on RepoTarget, not a separate field.** The remote host travels with the target enum rather than being looked up separately. This means any code with a `RepoTarget` has everything it needs for SSH commands.

- **Unified quick actions bar.** Rather than showing a "remote actions unavailable" notice, the same bar works for both local and remote — terminal/IDE buttons dispatch to the right launcher, and "Copy SSH" replaces "Reveal in Finder."

- **Shell escaping via single-quote wrapping.** `sshCommand` uses `ssh -t host 'cd path && exec $SHELL -l'` with single-quote escaping. Simple and correct for paths with spaces/special chars.

- **Default flow changed to ship-roadmap.** New waves get the full pipeline instead of a single `design` step. This assumes the roadmap flow is ready for general use.

## How it fits together

`RepoTarget.remote(path:host:)` is the spine — it carries both the server-side path and the SSH host. Views read `repoState.repoTarget?.remoteHost` to decide local vs remote behavior. `TerminalLauncher` methods accept an optional `remoteHost` and branch internally, keeping call sites clean. Error mapping flows through the existing `WaveServiceError → DisconnectReason → ConnectionState` chain.

## Risks and bottlenecks

- **SSH command injection.** `sshCommand` concatenates the host directly into the command string. If the host ever contained shell metacharacters, this could be a vector. Currently safe because hosts come from `ServerConnection.host` (user-configured connection settings), but worth noting for future API-driven host sources.

- **Default flow change.** Changing new-wave default from `design` to `ship-roadmap` affects all users. If `ingest` or `kickoff` steps have issues, new waves fail out of the box.

- **No remote worktree existence check.** `hasWorktree` is set to `true` for remote targets (line 320 of WaveDetailPanel). If the remote worktree is deleted server-side, the UI won't reflect it until a refresh.

## What's not included

- SSE/WSS smoke tests through Caddy TLS proxy — deferred to deployment steps 1-2
- Reconnect stability under real WAN conditions
- Studio auth (JWT) — step 3
