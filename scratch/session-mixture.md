# Session navigation and retirement

Implementation authorized by Jack Heart on 2026-10-01, including publication and landing.

## What to build

Make the sidebar a reliable way to reach current conversations, especially those waiting for Jack, and retire abandoned CLI conversations without a separate cleanup ritual.

## Intent and decisions

Jack: “The sidebar is really becoming a session bar now that we are getting repo and wave sessions - this will be how you navigate to those.”

Jack: “I dont think think we basically ever have to show non-started tasks in the left bar”; “ok to make you go to the wave to find those”.

Jack: “We should aggressively cull my sessions to only the ones that i actually still need.”

Required: filter to Sessions blocked on Jack; record interruptions; retain interrupted, unfinished Flow conversations; remove interrupted disposable orphans from the working set. CLI and Desktop use the same rules. Repository and Wave primary Sessions have durable purpose and must not be mistaken for disposable orphans.

## Placement

Unresolved. No exact Wave was supplied; do not infer placement from the screenshot's labels.

## The demo

The repository and its Waves provide navigation to their primary conversations and Wave planning pages. Started Task conversations appear beneath their scope. Unstarted Tasks appear on the Wave page only. A visible “Needs me” filter reveals matching Sessions directly with enough ancestry to distinguish them. Selecting a result opens that exact conversation.

Exit a disposable CLI conversation through Ctrl-C: it disappears from current navigation and counts after refresh. Interrupt a Flow review: it remains reachable and its Flow does not advance. Stop only a response: proposed behavior keeps the still-open conversation available. Exit-only is the implementation choice: stopping a response preserves the conversation.

## Current system and evidence

- `WorkspaceProjection.swift` groups Sessions under Wave/Task; Task rows carry counts rather than Session leaves. `WorkspaceTask.inWorkingSet` admits open Sessions or started unfinished Tasks.
- `PodiumModel.visibleWorkspace` filters only interactive/headless mode. `WorkspaceNavigator.swift` computes Task badges and orphan counts from supplied arrays; orphan search also affects its count. The menu has no attention filter.
- `orphanSessions(search:)` checks only absent Work/Wave. That cannot distinguish a repository primary from a disposable unbound conversation.
- Source `store/sqlite/sessions.rs` excludes `completed_at` records in ordinary inventory. `human_session::summary_surface` reports missing clients as unknown; `surface` can report a launched disconnected Session as closed. Neither definition alone establishes interruption.
- `journal/mod.rs` records interrupted Exec exit, but the hook cannot distinguish SIGINT, SIGTERM and SIGHUP. Provider turn interruption and conversation closure are separate events.
- Installed `lf 0.12.28` exposes an older API than this checkout. Its ordinary list includes disconnected Sessions labelled closed, and warns that four newer artifact IDs are omitted. Installed inventory is incomplete; source behavior must not be presented as deployed behavior.
- Under Jack's cleanup request, `lf session complete` retired 15 disconnected ordinary Sessions on 2026-10-01, preserving native history. A fresh list fell from 25 to 10: seven active conversations and three unresolved Ask/Flow boundaries. Those boundaries were not completed.
- Jack supplied a 2026-10-01 transcript showing three unassociated Ask/Flow Sessions that could not be located in Task rows. The live roadmap also reports LOO-329 as abandoned while its condition says “Waiting for your review”; a stale Flow cursor must not create a current attention obligation.
- Jack's Loopflow Dev screenshot shows a store compatibility failure (zero applied drafts; candidate requires seven). That failed development reader and the installed CLI inventory are not comparable snapshots. The UI must distinguish unavailable inventory from an empty or complete reading, with one concise error and expandable diagnostics instead of repeated raw logs.

## Data structures and key functions

Reuse AgentSession identity, Session events, current driver/provider generations, Flow membership, and primary scope. Add durable interruption evidence tied to the exact Session and driver; retain whether interruption ended a turn or exited the driver. Do not invent a second inventory or derive lifecycle from missing processes.

Rust projections:

- `session_attention(session, evidence) -> Option<AttentionReason>`: unfinished Ask/Flow review, explicit ready-for-review, or a recorded completed interactive turn awaiting reply; unknown liveness alone is not attention. Native provider questions without a recorded boundary remain unknown rather than inferred from idle CPU.
- `list_attention(store, filter) -> Result<Vec<SessionRecord>>`: bounded metadata pages filtered through the same attention projection before pagination.
- `finish_session_driver(session_id, driver, outcome) -> StoreResult<()>`: generation-fenced evidence; stale/repeated drivers cannot write; retirement never fabricates successful completion or releases a Flow/Ask.

Expose the projections through SessionRecord with matching Rust/Swift DTO fixtures. Reuse the same Rust state derivation for list and detail. Swift projects one filtered Session set into rows, badges and section counts. Search intersects with attention; required human interaction remains discoverable even for a headless-origin Session.

Task status, CLI attention and Desktop attention must read the same current obligations, preserving exact Session destinations and recording unresolved association. Current terminal Task status takes precedence over stale review positions when deriving Task obligations; an independently unresolved Session remains discoverable separately. Reader failures or incompatible runtime selection cannot produce a reassuring zero count.

## Constraints and forbidden outcomes

No transcript deletion. No automatic review completion, Flow progression, or blocked-caller release during culling. No stale driver closing a resumed conversation. No absence-of-process inference masquerading as observed Ctrl-C. No Task backlog in the sidebar. No hidden closed Sessions inflating counts. No repository primary classified as an orphan merely because it lacks a Task/Wave.

Keep started Task navigation available when no conversation currently needs attention; filtering must not discard the selected pane or change durable ownership. Retain unknown historical evidence explicitly.

## Delete — do not maintain

Replace the conflicting state branches in `human_session::summary_surface` and `surface` with shared lifecycle projection. Replace the Work/Wave-only orphan predicate. Replace Task-count-only navigation where it hides the actual conversation destination. Update their exclusive expectations in `WorkspaceNavigationTests.swift` and `SessionControlsTests.swift`; preserve ancestry, incomplete planning, selection, and headless visibility coverage.

## Internal slices

One coherent PR, implemented in internal slices: lifecycle, visibility and counts must agree at shipment.

1. Reconcile state and retirement authority; durable interruption evidence, shared projection, minimal migration if needed, lifecycle and DTO proofs.
2. Session destinations for repository/Wave/started Task scopes; separate disposable orphans; remove backlog rows.
3. Needs-me filtering and matching counts, including search, collapsed ancestors, empty results, and retained selection.

## Done when

Gate runs `uv run python scripts/test.py --reuse-passing`: affected Rust and headless Desktop suites pass. Focused lifecycle tests prove CLI interruption retirement, Flow/Ask preservation, primary retention, restart races and retained history. Navigation tests prove direct attention destinations, consistent counts, and no unstarted Task rows. Provider tests distinguish stopped turns from driver exit. Visual judgment belongs to demo, not an unattended check.

Include regression cases for an abandoned Task with a retained human-review cursor, an unresolved unassociated Ask, and failed inventory loading. CLI and Desktop must agree on attention when pointed at the same runtime and store.

Current work: all three slices implemented and reviewed; landing remains. Review repaired closed-as-disconnected fixtures and a macOS canonical-path planning fixture.

Check: affected gate passed architecture/website; Rust fmt/clippy pass, materialized Rust matrix 2,063 passed with two fixture failures repaired and Flow/PR rerun 53/53 passed; headless Swift 314/314 and multiplatform boundaries pass.
