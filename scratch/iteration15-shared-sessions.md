# Shared Session contract — 2026-09-24

## Change

`SessionRecord` now carries required action descriptors (kind, label, help,
optional unavailable reason) and an optional Work display path. Rust projects
Open, explicit Move here, Complete, Approve and Iterate from the existing Session
boundary. Both CLI text and Mac controls consume those values. Stored typed
ancestry supplies `product / Desktop / LOO-291`; unavailable Work is named
explicitly, and unattributed repository conversations have no Work path.
No roadmap read was added for labels.

Reconciled local main `88cf10641` and the exact LOO-284 row in the iteration 14
snapshot before changing the API. Neither supplied this contract; that Task had
no runtime or active PR. This implementation remains on LOO-291's existing
branch. No other Task, controller, sibling checkout or PM record was changed.
This is not a claim that all LOO-284 acceptance evidence is complete.

Deleted Swift's kind/state resolution matrix, replacement inference and
`FlowResolutionAction` label enum. A Session's actual window-local surface still
controls local presentation; shared provider activity alone no longer creates
a `.live` boundary pane. Prepared commands, opening/errors, completion errors,
retained layouts and explicit transfer preserve their existing owners.

Review found two concrete policy mismatches and corrected them:

- Rust previously accepted FlowStep approval before Ready, although Swift
  disabled it; Iterate was also enabled before Ready. The accepted contract
  requires Ready for both. The operations boundary now rejects premature
  decisions before stopping a client, and settlement rechecks the same policy.
- Iterate requires a preceding autonomous skill. Its projected reason and
  execution use the existing flow lookup; a Ready first human node cannot
  advertise a legal Iterate. Approval remains available.

Complete and ordinary interactive Open also validate the shared action policy.
`--json --replace` still prepares without stopping a client. No new persistence,
Session lifecycle, provider ownership rule or automatic transfer was introduced.
The shared JSON change requires CLI and Mac to be built together; no permissive
DTO defaults or compatibility path was added.

## Proof

Receipts and exact source/binary hashes are in
[iteration15](configured-ui-evidence/iteration15/).

- Final Swift focused command selects SessionsStoreTests, the two Session DTO
  fixture tests and `sessionRowRestoresWorktree`: **14 tests pass**. Covers
  explicit Move here, prepared launches, failures, resolution and real four-PTY
  shell attachment/focus/draft/layout retention. The separate existing
  `workspaceRetainsNativeSplit` test passes both completion timing cases,
  including repository navigation and direct Session surfaces.
- Rust Session projection tests: **10 pass**, including the shared twelve-case
  action fixture, round trips, missing Work labels and Iterate without a
  predecessor. Premature decisions pass through both operations and settlement
  rejection without modifying the playhead or adding a Steer. Ready approval
  and Ready iteration each pass their existing behavioral test. An earlier
  `human_` filter also passed 35 tests; it predates the final consolidation and
  is retained only as an earlier receipt.
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, the matching CLI
  build and `git diff --check` pass. SwiftPM compiled the matching app during
  focused testing. No broad affected-suite or repository test gate ran.
- Read-only queries against the explicitly aligned configured development Home
  returned four real Sessions. The production Swift DTOs decode and round-trip
  all four; CLI action labels/reasons agree. The exact LOO-291 Work reference
  has `product / Desktop / LOO-291`. These are four configured interactive
  records, not twenty comparisons or an all-kind configured trial.

The signed disposable review bundle is
`/tmp/loo291-iteration15/Loopflow Shared Sessions.app`, using this checkout's
matching CLI. Its read-only launch used the explicit Home/environment in
`read-only-app.swift`, with no fixture mode or installed-app replacement.
Runner AX trust was true, one owned AX window existed, and the app was inactive.
The expected count control was not observed within 12 seconds. That is a failed
UI observation with cause unestablished; it proves neither a permission failure
nor successful configured rendering. PID 99989 accepted termination and was
absent afterward. No keyboard, pointer, AX action, provider launch, Session open,
transfer or resolution occurred in this probe. The ongoing human demo is untouched.

## Remaining

Prove the new controls through configured Ask/FlowStep interaction and the
remaining nested keyboard/draft path. Obtain the human-selected external Task
and exact authorized directive edit; run the ten external trials and twenty
long-lived-registry trials against published scoped budgets. Keep promoted-Ask
caller release and the original Project evidence windows separate. This slice
supplies the previously absent shared contract, not full LOO-291 completion.
Nothing was published, landed or marked complete.
