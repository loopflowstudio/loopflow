# Iteration 14 review — 2026-09-24

The exact Session picker and nested workspace slice advances the accepted design.
Its configured receipt holds at its stated boundary. A fresh native proof passes
for row return, AppKit focus, retained input and responding PTYs. No production
defect or source correction was established. LOO-291 is not ready for publication.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Exact picker selection | Open the selected Session without replacing a local client | Existing Session ID reaches the existing opening action; multi-item actions expose that ID | Iteration 14 configured probe/log; all five recorded Swift hashes and both app/CLI binaries still match | pass, configured |
| Nested workspace retention | Return to the same checkout and companion panes | Two groups/four panes and exact provider/PTY receipts survive details and Session return | Inspected configured assertions and receipt | pass for pane/process retention |
| Focus, draft and responses | Return to the actual native terminal with unfinished input intact | Row switches, outer split and hidden-slot return preserve surface identity and AppKit first responder; four cat PTYs answer | Fresh `sessionRowRestoresWorktree`, one test passed | pass, native fixture; configured keyboard/draft gap |
| Completion | Resolve only the selected Session and preserve Task/companion terminals | Owned Session disappears; four original PTY children remain; Task remains incomplete | Prior configured UI completion; fresh read confirms all six trial Sessions remain absent | pass at recorded boundary; configured shell response gap |
| Planning/Session identity | Shared Project/KR/Task evidence joins exact typed Work | Live Desktop definition, two KRs, Task directive and exact human Session association are present | Fresh real CLI roadmap/Session reads | pass, read-only API |
| One reading/workspace owner | No parallel inventory, hierarchy reader or terminal registry | Podium owns shared reads; one root registry retains checkout workspaces and window-local surfaces | Source trace and negative searches | pass for consolidation |
| Shared Session action contract | Consume LOO-284 legal actions and display path | Fields remain absent; Swift still branches on kind/state for actions | Rust/Swift DTO and reachable action controls | gap |
| Complete Task proof | Authorized external edit, ten external trials, published budgets/twenty trials | Editor Cancel/rejection and bounded local/configured receipts exist; required external/budget evidence does not | Directive, design and evidence ledger | gap |

## Demonstration and evidence limits

Ran:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter WorkspaceNavigationProofTests/sessionRowRestoresWorktree
```

One Swift Testing test passed, exit 0, at 07:35 PDT. The XCTest wrapper's zero-test
line is followed by the actual one-test Swift Testing result. The fixture mounts
the production SessionsView, registry and Ghostty surfaces. It switches two exact
Session rows across checkout groups, adds an outer split, closes/restores a slot,
checks actual first responder and the same four surface handles, then reads the
retained draft and companion responses from four real `/bin/cat` PTYs. Session
records and registry transport are fixtures; input enters through Ghostty's API.
This proves neither configured provider keyboard delivery nor shell commands.
[Native log](configured-ui-evidence/iteration14-review/native.log).

Fresh `target/debug/lf roadmap --all --json` and `session list --json` used the
same explicitly aligned Home as the configured trial, from this Task checkout.
The snapshot contains Desktop's definition/two KRs and LOO-291's directive.
Planning ID `ee671927-255f-41e6-8429-b830d59cc1de` maps to typed Task Work
`task_2aa71a7e36fe416d8a721e2b2f7c54e7`, which matches the existing human Session
`run_643d86d70d5b4109a460e80a87a97533`. The Task is incomplete; its descriptive
condition is `blocked`. No execution conclusion is inferred from that condition.
Four Sessions are returned overall. None of the six retired trial IDs is present.
These are fresh read observations, not a new configured UI trial or provider
liveness check. [Receipt](configured-ui-evidence/iteration14-review/receipt.json).

Inspected the implementation trial's exact picker actions, provider ancestry and
receipt comparison, pane-count/ancestor-selection assertions, completion and
cleanup. Its four retained process receipts identify `/usr/bin/login` PTY
children. An active pane accessibility label does not prove keyboard focus, and
retained login processes do not prove shell responsiveness. Those limits are
already stated accurately in `iteration14-proof.md`; this review preserves them.
All eight source/binary hashes in that trial's receipt matched before verification.
No retired probe was replayed or existing client opened, moved or resolved here.

## Source and architectural review

Read the directive, complete target, current slice, integration amendment and
Done When/forbidden outcomes. Inspected the unrestricted base-to-working-tree
diff at `/tmp/loo291-review14-tracked.patch` and compared its 207 sections with
review 13. There is one executable delta: the existing multiple-Session menu
buttons now carry `session-row-<id>`. Four scratch documents also changed; binary
patch formatting accounts for the other section differences. Prior executable
reviews remain applicable. Inspected the new untracked configured probe/log/
receipt separately and checked current native-test assertions.

Traced WorkspaceNavigator → SessionsView.openSession → shared Session record /
window-local terminal association → retained checkout registry → outer layout /
inner multiplexer → native focus. The picker passes the existing record unchanged.
Selection places/focuses its existing shell before asynchronous preparation;
checkout location is not used to infer terminal ownership. Hidden outer slots
retain their inner layout. No second opening path or policy was added.

Searches find one Podium caller each for roadmap, process Activity and Sessions,
and one production root SessionsWorkspaceRegistry. SessionScope, SessionContext,
SessionGroup, SessionRowItem, PodiumConsole, the hierarchy loader and obsolete
providerLaunch kind remain absent. Planning and Session attribution still join
through typed shared Work identity; unmatched Sessions remain reachable. The
change adds no writer, persistent field, fallback reader or global surface owner.

The authority reduction is nevertheless incomplete at the shared action boundary:
SessionRecord in Rust and Swift has no legal-action/display-path projection.
SessionsView still selects FlowStep controls and Complete availability from
kind/state (including the ready Ask check). Moving those branches to a helper
would not satisfy LOO-284. This is existing inherited behavior, not a defect
introduced by the accessibility change, and remains a material full-Task gap.

## Disposition

No executable edits or broad gate. The focused native proof and `git diff --check`
pass. The current slice is coherent and preserves a path to the full design.
Do not spend the next pass repeating picker, Cancel or injected-rejection trials.
Integrate the shared LOO-284 action/display contract and remove the local policy
together; complete configured nested input/response proof when its exact input
boundary is available. The human-selected external Task and authorized directive
text remain unprovided, so no external edit/trial may be invented. Publish scoped
paint/interaction budgets before scoring the required long-lived-registry trials.

No publication, landing or Task completion. The
[review-slice skill](/Users/jack/.agents/skills/review-slice/SKILL.md) calls for
publication “When all applicable `Done when` claims hold and the slice is
coherent.” Shared Session integration and the outstanding configured/full-Task
claims leave that condition unmet.
