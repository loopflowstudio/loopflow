# Configured planning review — 2026-09-24

The slice advances the accepted design. The configured app now demonstrates exact
planning text, Task-to-Session access, the shared Work display path and a Project's
explicit no-Session state. The earlier locked-desktop observation is superseded
for this runner at 19:38–19:42 UTC. No product correction was needed in this pass.

## Scope and review

Read the Task, current design's slice/Done-when/forbidden outcomes, product memory,
implementation 16 and review 15. The complete target retains unified A/D navigation,
shared planning and Session authority, native checkout workspaces, scoped fresh
conversations and authoritative directive editing. Ten human-selected external-work
trials, an authorized edit and twenty trials against published scoped budgets
remain required. Earlier cardinality/Task-first proposals are not authorization
to impose those policies.

Recovered the unrestricted tracked base-to-worktree diff in
`/tmp/loo291-review16-tracked.patch`: 209 sections; 202 identical to review 15's
snapshot, seven changed, none removed. Reused that review for unchanged sections.
The executable delta is review 15's resolution-error correction and native test;
the other changes are README and evidence/design updates. Inspected those changes,
the untracked iteration-16 comparator and probes, and this review's new probes.
All 11 implementation-16 and 12 review-15 recorded hashes still match, including
the configured executable and CLI. Existing dirty/untracked work is preserved.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Configured availability | Actual window and correctly scoped count | Real AXWindow and matching count | Fresh read-only launch: 6 Sessions at 1,805 ms; final planning launch: 8 at 1,942 ms | pass for accessible count; populations differ |
| Planning details | Current directive, Project definition and KR proof | Exact Task selection presents shared text | Final configured probe compares directive, definition and both KR texts/verdicts with captured CLI planning | pass for LOO-291 |
| Session association | Exact shared identity, readable path, explicit absence | Task access retains Session ID; Project has no Session | Final probe checks exact Session-row ID and shared-path help, then selects Project and observes no-Session text | pass for access presentation; no Session opened |
| Freshness | Distinguish reading time from source freshness | Visible Planning read timestamp, source-sync limitation in help | Configured timestamp observation and toolbar source | pass for honest read timestamp; no fresh Linear sync claimed |
| Shared contracts | Rust owns facts and legality; Swift consumes them | Production decoding preserves all compared values | Fresh 158-record comparison: 5 Projects, 145 Tasks, 8 Sessions; 7 KRs and one exact Task join | pass for observed population; all Sessions interactive |
| Rejected decisions | Retain terminal and error through polling/retry | Resolution error remains independent of opening state | Reviewed correction; unchanged hashes and prior 13-test native/store pass | pass locally; no fresh test or configured Flow decision |
| Native interaction | Retain nested panes, focus, drafts and responding shells | Existing retained owners and earlier bounded receipts | No keyboard/provider interaction in this review; app remained inactive | gap for remaining configured nested input and Ask/Flow controls |
| External workflow | Ten human-selected trials, one authorized edit round trip | Editor/shared PM path exists | Workflow and intended edit remain unprovided; prior editor proofs retain their limits | gap |
| Performance | Published scoped paint/interaction budgets and twenty registry trials | Existing measurement paths | These observations measure AX availability only; prior six-run comparison is narrower | gap |

## Configured evidence and probe corrections

Used `/tmp/loo291-iteration16/Loopflow Proof.app`, the matching local CLI and the
same explicitly selected development Home. No installed binary or human demo was
replaced. The runner was Accessibility trusted and each launch exposed AXWindow;
the owned app remained inactive. Targeted AX presses selected only Work and list
presentation. No keyboard input, provider launch, Session open/Move here/resolution,
editor interaction or PM write occurred.

The first planning attempt proved exact text but sought a Session list row after
full-width Task selection hid the list. Two subsequent probes exposed another
assumption: SwiftUI reports some toolbar/detail controls with the parent's
`sessions-surface` identifier. The retained control dump contains their actual
labels, including Show work list and the selected Task's Session title. The final
probe uses those observed unique button labels, then still requires the exact
Session-row ID and shared Work-path help in the visible navigator. These were
probe lookup corrections, not missing product controls or permission failures.
Failed attempts remain recorded; their partial text observations are not full passes.

The [final configured probe](configured-ui-evidence/iteration16-review/planning-label-probe.swift)
passes, exit 0, including unchanged before/after Session ID sets:
[log](configured-ui-evidence/iteration16-review/planning-label.log).
Launch-to-count was 1,942.2 ms and post-selection-to-directive observation 452.9 ms.
The latter starts after AXPress returns. These include polling/traversal and prove
neither rendered paint nor interactive terminal readiness, p95 or cold-cache budgets.
All five owned app PIDs were absent after termination. The
[158-record comparison](configured-ui-evidence/iteration16-review/comparison.json)
uses fresh cached reads through production Swift types; it supplies no external
workflow or additional UI-trial credit.

## Architecture and disposition

Re-traced shared action projection and pre-stop/settlement checks, retained opening
and resolution errors, and planning-to-Session selection. Negative searches retain
one Podium caller per inventory read and one production root workspace registry.
SessionScope/Context/Group/RowItem, PodiumConsole, FlowResolutionAction,
`_loadHierarchy` and the obsolete providerLaunch kind remain absent. Swift consumes
action descriptors; it does not regain a legality matrix or labels-only reader.
Planning identity, runtime Work, actual terminal attachment and local surface
ownership remain separate facts. No new store, authority or workaround was added.

No executable product edits or test reruns were warranted. `git diff --check`
passes; the [receipt](configured-ui-evidence/iteration16-review/receipt.json)
records unchanged source/binary hashes and all five owned apps absent. The useful correction
is to the evidence and next direction: do not carry the historical desktop lock
as a current blocker. Next, establish exact native focus in the configured runner
before fresh owned Ask/Flow and nested-input proof. Preserve existing user clients.
The external workflow/edit still requires the human's selection; do not substitute
Loopflow dogfood. Define and measure the remaining scoped budget series separately.

Publication is withheld under review-slice's requirement that all applicable
Done-when claims hold. This configured planning pass does not satisfy the remaining
interaction and full-Task obligations. No publication, landing or Task completion.
