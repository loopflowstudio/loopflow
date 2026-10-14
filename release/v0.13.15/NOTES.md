# v0.13.15

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.15 makes it easier to return to the right conversation and carry it forward with current settings or a different provider. Session launch settings now apply consistently across CLI, Desktop and headless work, while native prompt delivery handles large requests through terminal wrappers more reliably. Readable Task labels make work easier to recognize, and disposable offline data clones let contributors explore retained planning and history without modifying the source installation.

## Return to the conversation and keep going

Resuming work now follows its Task and checkout context. Stopped Sessions pick up current launch settings, and an explicit provider switch carries earlier conversation text into a fresh native conversation while preserving the Loopflow Session.

- `lf resume` prefers the active Task's conversation, then interactive recency within the worktree. From the main checkout, recency spans the repository's worktrees. Explicit `--task` and `--wave` selectors override discovery.
- `lf connect` selects a Session ID, Task or Wave and prepares a primary conversation when needed. Desktop uses the same preparation. Add `--prepare` to return connection data, including remote SSH commands, without launching a provider.
- Stopped Sessions reload settings from their saved checkout, including permissions and Chrome integration. Same-provider continuation retains native history and account; live connections keep their existing settings, and historical replay retains captured inputs.
- Use `lf resume -a <agent>` or `lf connect <session> -a <agent>` to change providers. The Session keeps its identity, name, Work and feedback. The new provider receives a conversation log and opening/latest excerpts, without reusing the earlier provider's native token or account.
- If the new provider fails to start, retry `lf connect <session>` without `-a`; the saved continuation still targets the selected provider. Earlier native conversation selections remain in Session history.
- Native Codex receives instructions, hooks and the exact first request through its provider protocol; Claude receives its first request through stdin. Preparation feedback and retained failed-start input improve recovery, and normal disconnects no longer report a broken pipe or replay the first turn. Codex retains terminal-owned approvals, native tools, workspace roots and wrapper hooks.

## Recognize work across Sessions and checkouts

Tasks now share a readable label across CLI and Desktop views, Session history, automation and reports. Generated names follow that label without changing durable Task identity or moving existing work.

- Known Linear identifiers remain usable offline. Local-only Tasks use short hexadecimal codes that start at six characters and extend when needed to distinguish collisions, including completed Tasks.
- New branches combine the author, Task identifier and a short title, such as `<author>/eng-123-fix-login`. Explicit workspace names remain supported; an unrelated existing branch produces an actionable error.
- Generated Session names begin with the Task label. Identifier updates, binding and attachment repair outdated labels while preserving manually assigned names; passive reads do not rename Sessions.
- Label changes leave recorded branches and checkout paths intact. This naming change requires no schema migration.

## Explore real history in a disposable offline Machine

A raw database copy can retain credentials, provider paths and live-client claims. `lf clone create`, `run`, `show` and `dispose` provide an isolated development path that preserves useful planning and history while removing copied connections to the live installation.

- Clone creation backs up SQLite under source write protection and applies canonical migrations and this build's drafts only to the destination. Ordinary database opens remain validation-only; draft SQL gains no published migration provenance.
- Clones retain planning identities, Session history and Task-primary selection, while removing copied credentials, routes, PIDs, endpoints and attachment tokens. Codex and Claude JSONL archives can be supplied explicitly.
- An inherited macOS sandbox confines descendant writes, networking, Unix sockets and process control. Source repositories remain read-only, and Desktop helpers and embedded-terminal environments stay pinned to the clone.
- Browse retained history or supply offline stand-in executables for fresh disposable conversations. Copied provider continuation is unavailable.

## Operational notes

- **Update Session callers.** Replace `resume ID` with `connect ID`; use `--message TEXT` for headless continuation. Add `--prepare` wherever `connect --json` previously meant preparation only: `--json` now controls output format only. Primary-conversation replacement is `session new`; takeover of the same conversation is `connect --takeover`. The old spellings have no compatibility aliases.
- Provider import supplies a transcript, not a summary. Long conversations are excerpted in the first turn, so the new agent must read the log for the middle. OpenCode import requires `opencode` installed and runnable from the Session's working directory. Directly opening a saved native session with a mismatched provider still fails; use the Loopflow Session continuation commands instead.
- **Offline clones were verified on macOS 26; macOS 15 is unsupported.** Creation probes sandbox capability before reading source data and refuses without an unsandboxed fallback. Refusal can leave an incomplete disposable directory. Native window/PTY draft switching and real-account continuation are not established by this release's clone evidence.
- Required execution boundaries remain independent of YOLO settings. OpenCode still cannot satisfy managed Task execution requirements; its prompt transport is unchanged. Claude background/attach reconnect remains excluded from the verified native paths.
- Real Codex plain and wrapped terminal launch/reconnect and Claude large-message stdin delivery were exercised. Settings propagation and provider switching use stand-ins and simulated SSH; installed-app and visual Desktop acceptance remain unverified. Linux planning reconnect and shutdown acceptance remain CI-owned, and the supplied record does not establish a full hosted matrix pass for the combined release.

## Small changes

- macOS process observation uses the SDK kernel reader instead of privileged `ps`, retaining process birth and zombie evidence. Linux keeps its existing reader.
- Provider and relay sockets use the caller's temporary root so isolated clones can launch safely.
- Claimed Session inputs record the provider and model actually launched.