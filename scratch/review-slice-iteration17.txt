# Hierarchy study review — 2026-09-24

**Return to implementation.** The browser study demonstrates the proposed
navigation interaction with sample data. The accepted launch decision requires
an end-to-end native core that is not implemented yet. Prior app receipts cannot
approve the new composition. No production correction was warranted in this
review; corrected the stale next-step instructions in the Task design instead.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Study selection | Same Session and draft across full, compact and flat presentations | One selected ID and retained textarea per sample Session | Fresh headless Chrome check; compact and flat captures inspected | pass, simulation only |
| Study reachability | Same-named Sessions, upcoming Tasks, non-Task conversations and unknown ancestry stay accessible | Six Session leaves; compressed Project details remain selectable; folding hides descendants | Browser checks cover separate drafts, Project details, fold/reopen, upcoming Task and unknown ancestry | pass for this population |
| Native outline | Repository through Session appears in one compressible navigator | Repository/status banner, All work and Task-view controls remain; no native presentation menu | `PodiumView.swift:123`, `SessionsView.swift:453`, `WorkspaceNavigator.swift:44` | gap |
| Task workspace | Restore retained choice; initially Monitor; combine Monitor, Session and shell with existing split/zoom | PaneContent has only empty, session and shell | `MultiplexerLayout.swift:13`; retained registry in `SessionsView.swift:44` | gap |
| Truthful Monitor | Exact Task attribution plus current Run liveness; old active Runs retained; unknown differs from empty | Activity nodes lack Run/Task join; checkout activity is used only by existing Task-list filtering | `top.rs:56`, `WorkspaceProjection.swift:21`, `runs.rs:15` | gap |
| Two experience measurements | Repeatable small/large native journeys, usable endpoints, latency/hitches, per-attempt results and measured budgets | Existing Session logging; neither named metric appears in Swift or scripts | Search for `hierarchy_interaction_ms` and `task_workspace_ready_ms` | gap |
| Shared ownership | Reuse shared planning/Session facts and retained window/checkout owners | One Podium reader per inventory and one root workspace registry; removed Session policy types remain absent | Source searches; 15 source/fixture hashes unchanged from review 16 | pass for existing foundation, not new pane behavior |
| Full acceptance | Human composition demo, original external trials/edit and long-lived-registry budget series | Earlier bounded receipts remain; new native behavior and required full trials unestablished | Launch decision and Task acceptance ledger | gap |

## Scope and source findings

Read the latest launch decision, current Task slice and Done-when/forbidden
outcomes, product memory, study source and prior review. Compared the unrestricted
tracked base-to-worktree diff with review 16: 209 sections, 205 identical, four
changed documentation sections. No tracked executable delta. Reused the prior
review of unchanged sections and inspected the new study and launch amendment
separately. Fifteen recorded source/fixture hashes still match. These checks
establish content continuity, not fresh native behavioral results.

The browser proof ran against the actual HTML in this checkout, using an owned
headless Chrome instance. [Check](configured-ui-evidence/iteration17-review/study-check.cjs),
[passing log](configured-ui-evidence/iteration17-review/study-check.log) and
[receipt](configured-ui-evidence/iteration17-review/receipt.json) preserve its scope.
Compact and flat captures show one row vocabulary and selection retained at the
same Session. The study compresses only known-complete singleton Projects. It
does not prove repository/Wave compression, incomplete live planning behavior,
native input focus, multiplexer retention, performance or human acceptance.

The production owners remain useful: Podium reads roadmap, process activity and
Sessions once; the per-window registry retains checkout multiplexers and one
surface pool. Searches find no SessionScope/Context/Group/RowItem,
FlowResolutionAction, requestedSessionId, `_loadHierarchy` or obsolete
providerLaunch kind in the inspected production paths. However, removal of the
competing navigation controls has not happened. The browser cannot supply that
negative architectural proof for the app.

Do not reuse `hasProviderInCheckout` or unresolved Session membership as Monitor
liveness. They currently support Task-list visibility, not exact Run attribution.
The Rust Activity projection lacks the required join. Ordinary `lf runs` scans
seven days before applying its fifty-Run budget; the budget preserves all
unterminated Runs within that window. Corrected the launch document's shorthand
accordingly. An older active Run can still disappear at the time-window boundary;
unfinished metadata is not proof of a live owned client.

## Next slice and disposition

Implement the single core already authorized by the launch decision:

1. Reshape the existing navigator into the full/compact/Session-leaf outline,
   preserving typed selection, contextual actions and unavailable ancestry.
   Compact first and one menu are settled implementation defaults.
2. Establish the shared exact live Task-Run projection and mirrors, then add
   Task-bound Monitor content to the existing pane tree. Prove old active Runs,
   same-checkout different Tasks, live waiting versus dead clients, deduplication
   and explicit unavailable evidence. Preserve each live terminal's pane reference.
3. Implement both native measurements alongside that interaction, before targeted
   optimization. Use fixed populations and retained real PTYs; retain failures,
   usable-endpoint assertions and rendering/AX limitations. Compile native and
   fallback paths and demonstrate combined panes with retained drafts/input.

The two optimization Tasks remain deferred until this core supplies contracts
and baselines. No second worker, competing Watch implementation or fresh design
approval is needed. Final human confirmation and the original ten external trials,
authorized edit and twenty registry trials remain explicit obligations; none
are replaced by the HTML checks. LOO-251 and the Project evidence window retain
their separate scope.

This review changes only working documentation and proof artifacts. No native
test rerun was warranted for unchanged production code. `git diff --check` passes.
No user client, installed app, PM state, publication, landing or Task completion
was changed. Publication is withheld under
[review-slice](../../../.agents/skills/review-slice/SKILL.md)'s instruction:
“When all applicable `Done when` claims hold and the slice is coherent, publish
or refresh the Task PR.” The native implementation and acceptance gaps above
do not meet that condition.
