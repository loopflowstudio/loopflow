# Iteration 7 independent review — 2026-09-23

Iteration 8 follow-up: [the rejection fix](iteration8-proof.md) reproduces error
loss during inventory reconciliation and passes the strengthened native proof.
The next configured attempt found the macOS desktop locked with AX trust granted;
no provider was launched. Historical failures below remain their dated evidence.

Disposition: the integrated slice advances the design, but publication is not
approved. This review records a fresh configured shell/navigation pass and a
later failing native rejection case. It does not replace earlier receipts with
an assertion that the integrated provider path has passed.

## Scope and source

Read the Task directive, current main-view design and lf-new integration design.
The current slice includes unified navigation, fresh scoped conversations, nested
checkout/terminal layouts and exact shell attachment. Mandatory Task creation and
one-current-conversation enforcement are superseded. Directive editing, shared
LOO-284 actions/display path, bounded conversation lifecycle, human-selected
external trials and published paint/readiness budgets remain full Task scope.

Recovered the full Task diff in `/tmp/loo291-review7-diff.json` and
`/tmp/loo291-review7.patch`: `truncated: false`, 11,366 lines, 129 sections.
Compared against iteration 6: 100 sections changed, 29 unchanged. Reviewed the
changed production paths and relevant native tests, with supplied scratch
artifacts providing historical context. Initial HEAD was `b12beba8b`; another
writer checkpointed the existing integrated work as `560783243` during review.
The subsequent shell-completion change and its test remain that writer's work.
This review removed its own overlapping test rather than retaining duplicate
coverage or overwriting the concurrent correction.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Configured planning/navigation | Actual directive, exact Session/no-Session access, A/D and details | LOO-291 directive and no-Session state visible; list/details/overview/terminal transitions work | Fresh external AX trial against integrated app and real registry | pass at this scope |
| Shell continuity | Navigation preserves input and native shell | Exact baseline sentence and navigation-retained sentence received by the same cat child | `independent-review/shell.log` | pass, configured shell |
| Shell-attached completion | Complete an exact attached Session while retaining its shell and other Sessions | Concurrent correction exposes per-Session actions over actual terminal attachments and reuses shared completion | Native success case passes, including continued shell input | pass, fixture CLI/real PTY only |
| Rejected completion | Visible rejection with retry and unchanged Sessions | Error text is intended to render from existing SessionItem error | Latest combined run cannot find `Completion rejected`; previous concurrent run passed | gap; inconsistent proof |
| Direct completion and repository return | Remove completed pane, preserve companion and Task | Both mounted completion timings pass in latest run | `workspaceRetainsNativeSplit`, two cases | pass, fixture CLI/real PTY |
| Provider continuation and timings | Integrated configured provider path and comparable paint/readiness measurements | Earlier exact provider and fixed-population count receipts precede integration | Existing iteration 7 binary-specific receipts; this trial adds one AX observation | gap for integrated provider/budgets |
| One authority | Shared planning/Session reads, retained native ownership | One Podium caller per inventory read, one root workspace registry; removed hierarchy/scope types absent | Fresh negative searches and Rust/Swift/main comparison | pass, source; LOO-284 still absent |

Paths in the matrix are under
`configured-ui-evidence/iteration7/independent-review/`.

## Fresh configured result

Ran the adapted [probe](configured-ui-evidence/iteration7/independent-review/shell-probe.swift)
against `/tmp/loo291-integration/Loopflow Integration.app`, executable SHA-256
`ca5ba3b0ab832c66973dd34943f0b026ca78c15cfde9fd21daa3d5be3d110090`.
Receipt-listed source hashes matched before this trial. Explicit Home variables
selected the installed development Home; deliberately unrelated `LF_WAVE_ID`
did not narrow planning. No fixture/capture mode was set.

The exact runner reported AX trusted, AXWindows success/count one, one owned
onscreen window, and active true. It observed the actual Task after 2,986.3 ms,
selected LOO-291, then used New terminal and normal native input. The child
received `Reply with only the continuity word from my first message.` exactly,
both before and across Show work list → Work details → All work → Return to
terminals. [Receipt](configured-ui-evidence/iteration7/independent-review/shell.log).
The owned app PID 24860 exited; no user Session was opened, moved or completed.
This is one AX-observation duration, not pixel paint, provider readiness, p95,
or a fixed-population comparison. It predates the completion correction.

Inspected the integration's light and dark captures: primary labels, search and
toolbar are readable. These remain static rendering evidence, not provider or
terminal-scroll interaction. The older missing-Session observation is explained
by Home mismatch; generic AX-unavailable language is no longer appropriate.

## Completion finding and verification boundary

New conversation opens a shell pane. Before correction, completion required a
`.session` pane, so these conversations could focus locally but had no Complete
action. Another active writer independently added the same correction and a
broader native test while this review was developing its regression. Preserved
that correction: exact attached interactive Sessions become individually named
actions when multiple share the shell; no arbitrary first Session is selected.
Completion retains shell identity, reuses SessionsStore.complete, and resets the
in-flight flag so a later conversation can also complete. No shared API changed.

This review's final focused command was:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'WorkspaceNavigationProofTests/(shellSessionCompletion|workspaceRetainsNativeSplit)'
```

It exited 1. Three of four cases passed; rejected shell completion failed because
ViewInspector could not find the expected rejection text. The direct completion
cases passed. [Failing receipt](configured-ui-evidence/iteration7/independent-review/completion-failure.log).
The other writer's immediately preceding identical two-test/four-case selection
passed: [prior receipt](configured-ui-evidence/iteration7/independent-review/concurrent-completion-pass.log).
Do not report this selection as consistently green or dismiss the failure as a
production or automation defect without identifying the cause. No retry was used
to erase it. The overlapping added regression was removed before this final run.

Next bounded work: establish whether the rejected state disappears or its mounted
error presentation lags in this test, then correct the actual boundary and retain
one deterministic assertion of visible failure and usable retry. Do not weaken
the test to provider success or mere mock invocation. After that, demonstrate
New conversation and its completion in the configured integrated provider path,
and record terminal-scroll and paint/readiness evidence against exact binaries.

Negative searches found no SessionScope, SessionContext, SessionGroup,
SessionRowItem, PodiumConsole, PodiumSurface or _loadHierarchy. Current main still
lacks terminal_ids; this branch's Rust/Swift DTOs include the complete attachment
contract. Neither has the promised shared legal actions/display path. The
completion correction adds no new inventory or writer and preserves the existing
policy pending LOO-284; moving that policy locally would not close the requirement.

`git diff --check` passes. No broad gate, publication, landing, Task completion,
PM write, external-product trial or installed-app replacement occurred. Under
review-slice's condition, “When all applicable Done when claims hold,” the current
proof gaps require another bounded slice rather than publication.
