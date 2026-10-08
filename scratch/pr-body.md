New Sessions take a short name from the requested work, making concurrent conversations easier to distinguish. Interactive lf launches put the Task identifier first when bound, and cmux workspace and tab names follow Session renames.

## What changes

- Share request-based names across interactive and headless lf Sessions. Keep human names and teach self-renaming through shared operating context.
- Set the terminal title before provider launch; update cmux through its control channel. Driver replacement stops observation, and missing, failed or stalled host commands leave the Session running.
- Install native naming hooks for new plain Claude conversations and shared-server Codex TUI conversations. Preserve existing settings, hooks, permissions and symlink destinations; retry failed hook writes even when the same binary is already installed.
- Remove random word pairs and resume-only native naming. Integrate main's native launch changes without restoring `--tui`.

## Coverage and limits

| Future start | Naming coverage |
| --- | --- |
| lf interactive, Task or taskless | Session name and initial terminal title; live cmux workspace/tab rename verified with a provider stand-in. |
| lf headless, Task or taskless | Loopflow Session name, including captured Flow steps. |
| Plain Claude TUI or `claude -p` | Native request title through UserPromptSubmit. |
| Plain Codex TUI using its shared server | Native request title when the installed hook is trusted. |
| Embedded Codex TUI or `codex exec` | No supported naming endpoint identified for the active embedded engine. |
| Plain launches outside `.lf`, or with hooks disabled/untrusted | No Loopflow naming callback. |

The live cmux demo's Session list, workspace and tab agreed on `Plan store migration`, then `Release notes` after rename. The separate workspace remained unselected in both readbacks. This supersedes the earlier workspace-creation blocker.

Names begin as three-word request excerpts; generic opening words can still produce weak names. Existing conversations are outside scope, and concurrent manual rename preservation remains unproved. Other terminals pick up changed names on reconnect. Rendered sidebar/window-bar agreement, latest-message presentation, herdr, configured shims and Jack's installed setup remain unverified.

## Checks

- Affected runner: architecture, formatting, Clippy and all 76 website checks passed. Python had 407 passes and one macOS nested-sandbox refusal; that test passed directly under its own network-denying sandbox.
- Release-shaped Rust suite: 2,288 passes and one stale prompt golden in its captured tree. All five snapshots were refreshed; the focused golden check passed. The added headless Flow naming assertion also passed after correcting its list filter. No full-suite rerun is claimed.
- Fourteen-launch public CLI/PTY fixture passed Task/taskless names, rename/reconnect, missing cmux, failure and timeout. Driver replacement and native-hook ownership checks passed.
- Fresh account-free Claude/Codex TUI and headless probes passed their documented contracts with local fake APIs and external-network denial. Codex exec completed with no name or shared socket, retaining the unsupported case.
- Disposable Docker promotion proved preview, failed-hook retry, existing settings/permissions/symlinks and repeated installation. CI now runs this public-entry proof. Final Clippy, Ruff and whitespace checks passed.

Fixtures establish naming transport and persistence. They do not establish model judgment, hosted CI success or rendered host acceptance. No provider accounts, existing cmux windows or Jack's native settings were changed by gate.
