# Conversations and worktree workspaces

## What to build

Start a conversation about the visible repo, Wave, Project, or Task in the configured destination, with ordinary companion terminals grouped by checkout and independently splittable worktree and terminal layouts.

Intent anchors:
- “Use the configured app or terminal”
- “new shell *which is a truly flesible shell*”
- “grouped together so when you switch between sessions you also switch your extra terminals”
- “the first is the woktree graph and then within the worktree you have the terminals. splits can be at either boundary level.”
- “amybe we do need unfiled tasks / taskless sessions”
- “so like hwoever zoomed in you are, the default prompt for the new shell changes”

The latest direction supersedes unconditional new-worktree-plus-design startup. This implementation uses **New conversation** and **New terminal**. Conversation creation is fresh and bounded; it does not replace a live Session or enforce a one-conversation-per-subject policy. Automatic Task creation and adopting an exploratory checkout remain open follow-ups.

## Placement

Implementation Wave/Project remains unspecified by the request. No ownership was invented. The committed `loopflow.main-view-task` implementation is integrated here; its unified list and optional sidebar own navigation. Original `loopflow.main-view` contains the earlier design. Neither sibling's working files were changed.

## Demo

1. From All work, New conversation opens a repository conversation. Select a Wave, Project, or Task and invoke it again: the prompt and binding follow that subject. All work resets the launch context to the repository while preserving the previous navigation selection for return.
2. lf chooses the configured app or terminal. Wave/Project prompts orient a present human; they do not invoke autonomous operating skills. Task context uses its existing owned checkout when available.
3. New terminal opens an ordinary login shell inside the active checkout. After an embedded conversation exits or hands off externally, its launch terminal remains a usable shell.
4. Open Sessions in two checkouts. Add companion shells, split terminals, switch between Sessions: the complete groups survive.
5. Split worktrees from a worktree header; split terminals from a terminal header. Hiding a worktree retains its processes. Selecting an already visible worktree focuses its existing slot.
6. Manually launch lf from a new shell. Its Session resolves to that live shell; selecting it focuses the existing terminal. Another window or external app remains ELSEWHERE.
7. Current Wave navigation excludes abandoned/retired registrations. Planning reads work even when a GUI was launched from a Wave Run with cwd `/`.

## Data and key functions

- `ConversationScope` captures repo or typed subject plus checkout; `ConversationLaunch.arguments(lf:)` builds `lf --interactive [binding] : <prompt>` without destination overrides.
- `WorktreeLayout` has checkout/empty leaves and horizontal/vertical splits. `WorktreeLayoutStore.select`, `split`, `close` own the outer layout.
- `SessionsWorkspaceRegistry.workspace(for:)` retains each checkout's `MultiplexerStore`. One window owns the shared `GhosttySurfacePool`; no NSView is mounted twice.
- `MultiplexerStore.newShell(command:)` optionally starts a conversation before returning to a login shell. Undoing a close does not replay that command.
- Provider-client receipts record an optional terminal ID only when inherited `LF_TERMINAL_TTY` matches stdin's actual PTY. `SessionRecord.terminal_ids` projects active clients; `SessionsStore.localTerminal(for:)` resolves native identity. Cwd/title matching cannot prove attachment.
- `lf ls --current` and ordinary roadmap reads own current-Wave filtering. `lf work forget wave <id> --dry-run` previews removal of an empty, abandoned, disabled registration. History-bearing or authored Waves cannot be forgotten through this command.

## Current system reshaped

The old repository multiplexer mixed terminals from separate checkouts. Local classification only checked `.session(id)`, missing agents running inside `.shell(id)`. The old toolbar conflated conversation launch with an ordinary shell. GUI queries inherited execution scope, and `roadmap --all` still applied ambient Wave selection. Wave navigation displayed abandoned registry history as current work.

The implementation replaces those paths while retaining main-view's single shared Session/planning readings, every existing Session, and explicit Move here for external clients. Old already-running clients without terminal markers cannot be associated retroactively by guessing.

## Constraints and forbidden outcomes

- Subject attribution and terminal location remain distinct. Free shells can run arbitrary commands and change directory without moving groups.
- No duplicate native surfaces, forced provider replacement, hidden unmatched Sessions, or fabricated Tasks.
- No autonomous operating pass, issue creation, or chat delivery from opening a human conversation.
- No GUI-local lifecycle filter parallel to CLI truth.
- Do not bypass development Home isolation to delete live registrations.
- App-relaunch process/layout persistence and automatic Task creation/adoption are outside this implementation.

## Internal slices — one integrated PR

1. Shell/client attachment with PTY and native-surface proof.
2. Outer worktree slots and retained inner terminal layouts.
3. Scope-aware conversation and ordinary terminal actions integrated into main-view.
4. Current Wave reads and exact empty-registration cleanup.
5. **This slice:** integrated verification and review; record remaining live-path limits.

## Done when and evidence

Focused proofs exercise repeated PTY bindings versus mismatched/detached stdin; two shell clients local in their window and elsewhere in another; independent split levels; hidden group retention; scope after zoom changes; shell survival after initial exit; current Wave filtering and exact dry-run/deletion behavior; and roadmap `--all` from `/` with inherited Wave IDs. Compile both Ghostty SwiftPM and fallback Xcode paths.

See `implementation-evidence.md` for observed results and limits. Mocked argv and fixture screenshots do not prove real configured-app startup. Live deletion of `list` and `engbot` requires the updated installed CLI and remains outstanding.

Follow-ups, not filed: decide explicit new Task/worktree creation and recovery, adoption of an existing exploratory checkout, conversation lifecycle/cardinality, and app-relaunch persistence. Each needs its own design before implementation.
