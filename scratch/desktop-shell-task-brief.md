Jack Heart requested this Task on November 3, 2026 while reviewing shell behavior. Scope is local and remote development in Loopflow Desktop, with coordination for external terminal hosts. Mobile, Tailscale setup, a public hub and a new general network server are out of scope.

## Outcome

People open a shell for a Task and work on that Task's actual Machine, in its recorded checkout. Ordinary `lf ...`, `git ...` and shell commands execute there. Shell lifetime, recovery and termination are predictable; repeated use does not leak shell, SSH, PTY-owner or helper processes.

Jack accepts that lf can supply a tmux-like retention/recovery responsibility when that responsibility is justified. This does not select a second multiplexer, require every shell to run through lf, or select a daemon architecture. Design the ownership and retention contract before implementation.

## Existing evidence to verify against current main

Source inspection in checkout 2bc29a72e0 found:
- SessionsWorkspace.newShell in swift/LoopflowMac/Views/SessionsView.swift starts ordinary local shells directly, while remote Tasks invoke `lf --machine <id> --repository <id> task shell <task> --checkout <path>`. Explicit scripted shell commands currently reject remote Tasks.
- rust/loopflow/src/ops/task/workspace.rs validates the recorded checkout, delegates through transport where applicable, and launches the destination account's login shell there. SSH routing lives in rust/loopflow/src/lf/commands/ssh.rs.
- buildWorkspaceShellCommand in swift/LoopflowMac/Services/Ghostty/GhosttyTerminalView.swift runs the initial remote command and then starts a local login shell when it returns. Exit or failed connectivity can therefore leave a local prompt in a remote-associated pane. Fix that ownership confusion; never silently retarget a terminal.
- Local shell surfaces survive Task/tab switching and hiding. Closing retains one shell for Undo; a later close replacing that slot releases the old surface. Ordinary local shell lifetime is not the same as detached agent Session lifetime. Inspect MultiplexerStore.closeShell, SessionsWorkspace.closePane and GhosttySurfacePool.release.
- Existing detached owners in rust/loopflow/src/transport.rs retain execution, but this is not evidence that arbitrary local shells already survive Desktop exit or have useful discovery/recovery afterward.
These are code observations, not an installed end-to-end demonstration.

## Required design decisions

1. Define who owns the shell/PTY, input attachment, execution records, output history and presentation for local Desktop, remote Desktop and Rex. Compare existing detached lf owners, Desktop-owned surfaces, an independent desktop/host service, and Rex-owned terminals. State exactly what survives closing Desktop, service failure, network loss, logout, sleep and reboot. Moving retention into an app-owned server cannot by itself survive app exit.
2. Decide where invoking lf is valuable: Task/checkout validation, Machine routing, durable identity, detached ownership or recovery. Reuse the shared core; remove redundant wrapping rather than institutionalizing a CLI hop with no responsibility.
3. Specify hide/detach, close tab, Undo, explicit terminate, Task completion/archive, checkout removal and retention cleanup. Do not let the next unrelated close accidentally determine process lifetime. Keep long-running development servers, jobs, shell variables, foreground TUIs and unsent input in the analysis; silence alone is not abandonment.
4. Separate a live shell from a saved screen and provider conversation history. Killing and recreating an arbitrary shell loses state; do not apply agent idle-ending policy as though shell recovery were equivalent. Bound unwanted retention through an explicit policy and inspectable ownership, without silently killing useful work.
5. Coordinate with LOO-498's Rex workspace design. If Rex retains a terminal running an lf attachment and lf retains the underlying execution, identify which layer owns which fact, why both are necessary, and how they end. Consider delegating ordinary shell retention to Rex versus keeping a portable lf execution owner; choose based on the desired recovery/cross-client contract, not an assumption that two processes are inherently duplication. No competing cleanup authorities or second provider conversation.

## Observable acceptance

- Open Task shells locally and through an actual remote connection; confirm destination identity, cwd and ordinary `lf` behavior. Repeated open/reopen follows documented create-versus-reattach semantics.
- Remote disconnect, authentication failure, shell exit and stale checkout never expose a local prompt as that remote Task. Show an honest disconnected/ended state and deliberate recovery.
- Demonstrate the agreed lifecycle matrix: view switching/hiding, close and Undo, explicit termination, Desktop quit/relaunch, network loss/reconnect, owner/service failure, Task/checkout changes, sleep/logout/reboot. Distinguish process survival from restoration; do not replay commands to reconstruct a screen.
- Run repeated local and remote create/attach/detach/close/recover cycles. Inventory actual child processes, SSH connections, PTYs, owner sockets and retained metadata before/after; all intentional survivors are discoverable and terminable, and disposed work returns to the expected baseline. Verify cleanup after partial startup and abrupt failures.
- Protect running jobs and relevant interactive state. Ending a terminal must not accidentally terminate another terminal's work, and retaining one must not leave unowned helpers behind.
- Tests exercise these behaviors, with installed Desktop/SSH evidence for the actual lifecycle. Document capabilities and gaps; a mock or saved preview is not survival proof.

## Coordination and overlapping work

Product owns the desktop experience; coordinate execution internals with Infrastructure.
- LOO-498, Open and restore a Loopflow workspace in Rex, already owns the external-host feature. Extend/reuse it for opening one Task's preconfigured tabs/splits in a Rex session rather than duplicating that implementation here. Preserve its Rex-first/Herder-second direction and existing reserved product choices.
- LOO-593, Decide the Desktop server's roles and long-term architecture, currently specifies an app-owned server with no Session ownership. Jack's new question explicitly reopens retention placement for evaluation; record the tradeoff and coordinate any proposed boundary change rather than silently interpreting older text as either a veto or approval.
- LOO-709 / PR #1804 (https://github.com/loopflowstudio/loopflow/pull/1804) actively changes transport.rs, transport/lifetime.rs and the transport contract: detached agent idle ending, preservation of active work, editable drafts and pending approvals, saved-screen return, and removal of bulk resume are under ongoing implementation. Preserve accepted requirements and still-reserved review decisions; do not start a rival cleanup policy or rely on this checkout's old 24-hour default.
- LOO-714 / PR #1805 (https://github.com/loopflowstudio/loopflow/pull/1805) touches commands/ssh.rs for agent/settings selection. Preserve its routing and environment changes.
- LOO-711 / PR #1806 (https://github.com/loopflowstudio/loopflow/pull/1806) touches WorkspacePanesView.swift for Task progress/history. Preserve those presentation changes.
- LOO-672 owns Desktop conversation readiness/restart; share identity/recovery semantics without deciding its reserved review choices.

No implementation or worker launch is requested by this capture. Primary comparison sources: Rex detach/block-close contract (https://www.superlogical.com/rex/docs/reference/api), server modes (https://www.superlogical.com/rex/docs/linux/server-modes), and upgrade lifetimes (https://www.superlogical.com/rex/docs/linux/upgrading). Validate real supported host APIs before designing against them.
