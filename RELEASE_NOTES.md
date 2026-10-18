# v0.14.6

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.14.6 keeps ongoing work connected when terminals close, Tasks change Waves, installations advance, or CI takes longer than expected. Conversations gain a single opening command and live reattachment, Tasks can start without a Wave, and waited landing stays with pending checks and repairs through merge. OpenCode 2 integration also restores terminal use, tools, and conversation history.

## Return to the conversation you were working in

`lf connect` opens or creates the conversation for the current repository, Wave, or Task; `lf resume` is now an exact alias. Checkout ownership determines the destination, with explicit targets available when needed. Detached execution lets a replacement terminal reconnect without resubmitting the original input. (#1581, #1585)

- Use `lf connect --fresh` to start another conversation while preserving the previous client and history. Ordinary connect then selects the new conversation; opening an exact Session leaves the Task's primary unchanged.
- Use `lf connect --prepare` to prepare a conversation without launching a provider. First creation applies the matching scope's session skill.
- Use `lf session attach SESSION` to rejoin live execution, `--observe` to watch without taking input control, and Ctrl+] to detach. A controlling attachment replaces the previous viewer's input authority.
- Sessions, Flows, and Task shells retain their owner across terminal loss. Reattachment restores terminal text, supported input modes, and up to 2,000 scrollback rows.
- Desktop and remote opening use the same connect command.

## Capture Tasks now, organize them later

Tasks no longer need a Wave or an automatically created inbox. Creation defaults to a standalone Task in the repository, and membership can change after work starts while retaining the conversation, checkout, PR, and running work. (#1589)

- `lf task create --title 'Fix preview refresh'` creates a standalone Task, even from a bound Session or checkout. Only explicit `--wave` selects membership.
- `lf task list` and Desktop show standalone Tasks, including captures that have not started.
- `lf task move <task> --wave <wave>` joins or changes Waves within the repository; `--no-wave` removes membership. The selected Task and workspace remain in place, with historical execution and usage attribution preserved.
- Workflow selection follows Task → Wave → repository. Wave defaults survive chapter changes; a Task's captured graph survives later membership and configuration changes.

## Let delivery wait for real outcomes

`lf land --wait-and-fix` no longer ends delivery or starts a synthetic repair because a time limit elapsed. It keeps observing pending checks and live repairs toward verified merge, including slow repair startup. (#1586)

- Keep one repair responsible through publication, avoiding duplicate workers when its new head appears before the worker exits.
- Report a concrete startup failure if the repair owner exits before claiming its reservation.
- Preserve cancellation, provider errors, and Task completion boundaries: merge alone does not request completion.

## Use OpenCode 2 with tools and retained history

OpenCode 2 changed the launch, context, and history interfaces Loopflow depended on. This release restores full-screen terminal use, tool execution, headless results, and reopening completed conversations with native history intact. (#1590)

- Refresh Loopflow instructions and scratch context without rewriting stored user messages, and keep diagnostics away from terminal input.
- Preserve initiating-input attribution for results and usage across steers and attachment changes; reject failed or empty completions.
- Recover when the private server dies before conversation creation. After interruption, accept explicit new input; automatic crash continuation and synthetic replay remain unsupported.

## Operational notes

- **Command migrations:** replace `session ensure` with `connect --prepare` and `session new` with `connect --fresh`; `--choose` is removed. Unassociated linked checkouts require an explicit target. Task membership now uses `task move --wave/--no-wave`; Workflow position uses `task workflow move`. The old `refile` command is removed. (#1585, #1589)
- **Upgrades inside Sessions:** new launches retain a stable installed CLI entry for later commands, hooks, prepared connections, and Flow children. The main Machine ignores inherited `LF_BIN` overrides; private `LF_HOME` launches retain explicit executable selection. Installation does not restart existing processes. Clients launched before this change may retain old paths; after their provider exits, reopening the same Session refreshes hooks while preserving history. Live reattachment retains the provider's existing environment. (#1587)
- **Stored state:** migration preserves existing Task ownership and moves current Project workflow defaults onto Waves. It also removes `automation.timeout_reruns` and stored timeout counters, resuming landings held by recognized timeout reasons while preserving concrete blockers and merge intent. Pending checks or an unresponsive landing operation can now wait indefinitely; viewer and release-publication deadlines remain separate. (#1586, #1589)
- **Reattachment limits:** continuity requires the detached owner to survive. An unavailable owner or attachment timeout leaves execution uncertain and does not authorize replay. Images, hyperlinks, extended keyboard protocols, and exact restoration of both screen buffers are unsupported. Explicitly switching a structured Run to native UI may restart its provider and interrupt the turn; `session attach` never makes that transition. (#1581)
- **Acceptance status:** local native Claude reattachment preserved an unanswered interaction and provider identity. OpenCode 2.0.32 probes covered terminal, tools, and resume; configured-account checks demonstrated shell execution and remembered content in a disposable store. These do not establish the normal installed/shared-store path. Installed continuity, real SSH recovery, configured GitHub repair through merge, and native Desktop/provider-input continuity remain open. The supplied records do not establish a clean final combined-tree matrix; OpenCode also retains two unresolved leaked-handle warnings. (#1581, #1585–#1590)

## Small changes

- Task tickets resolve case-insensitively while retaining their recorded spelling. (#1585)
- Task creation selects its Project under the planning lock during chapter rotation. (#1590)
- Retained terminal attachments finish correctly on Linux after PTY descriptor cleanup. (#1581)