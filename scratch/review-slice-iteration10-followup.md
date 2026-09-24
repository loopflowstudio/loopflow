# Review disposition — iteration 10 follow-up

Iteration 11 resolution (2026-09-23): the human's explicit definition in
`task-views.md` authorizes checkout activity as Active membership. That supersedes
finding 1's proposed Task-attribution requirement; the ambiguous Running label is
now Provider in checkout. Finding 2 is corrected with a failing-before/passing-after
regression. See [implementation and proof](iteration11-implement.md). The original
observations below remain historical; configured/full-Task proof is still open.

**Return for correction; do not publish this working tree.** The committed
workspace advances the accepted design. Concurrent Active/All Tasks work changes
default membership and introduces a new activity join; the earlier configured
bundle and native receipts do not validate that change. Two source findings
below need resolution before its proof is meaningful.

## Findings

1. **Task execution is inferred from checkout coincidence.**
   `WorkspaceProjection.swift:67` sets `hasRunningProvider` solely by matching
   `task.reference.workspace.worktree` against provider paths. Podium discards
   provider identity when building that set; the navigator then labels the Task
   **Running** and includes it in Active. An unrelated or explicitly differently
   bound conversation launched in that checkout produces the same result as a
   Task-bound worker. The new fixture supplies only a path, so it cannot reject
   this counterexample. A verified live provider establishes process liveness,
   not Task attribution. Project activity through its shared Run/Work relationship
   before making a Task-running claim. Prove both a bound worker and an unrelated
   provider sharing its checkout. This follows the design's stable-identity rule
   and `.lf/directions/desktop.md`'s distinction between subject and location.

2. **Active can render healthy emptiness while Task evidence is outside the plan.**
   `WorkspaceNavigator.swift:78` checks unavailable/truncated `projects`, but
   neither that check nor `includes(wave)` accounts for `unavailableProjects`.
   With no planned matches, a Wave containing recorded stranded Tasks disappears
   and the list says **No active Tasks in this repository.** Its only diagnostic
   has just moved to that Wave's details. Preserve access to the Wave detail and
   distinguish incomplete association from confirmed zero matches. A focused
   case needs available/untruncated planning, one unavailable Project containing
   a Task, no planned matches and successful Session/activity reads. The existing
   diagnostic test still has a planned Session-bearing Task, so it does not cover
   this case. The live Infrastructure diagnostic currently has no Tasks; the
   finding is a source-established boundary case, not a reproduced live incident.

The new filter's ordinary unavailable/truncated planning checks appeared during
this review and were inspected in their corrected form. Those are not reported
as outstanding defects. Active's exact product semantics remain open in
`task-views.md`; the identity and missing-evidence findings hold independently
of whether the human ultimately chooses this default.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Unified workspace | Exact Work/Session navigation, retained panes and explicit Move here | Existing shared readings, typed Session join and retained owners remain | Source trace; prior native and configured receipts | pass at recorded scope |
| Configured Task conversation | Fresh launch, local attachment, exact draft and shell after Complete | Prior signed bundle establishes draft/completion; implement 10 adds another launch/attachment receipt | Iteration 8 review and iteration10-implement.md | pass for recorded Task/terminal path |
| Session-row and nested return | Focus existing provider and preserve two inner layouts | Mounted four-PTY proof passes; configured row/nested trial still incomplete | Iteration 10 native receipt; configured probe stopped before row selection | local pass / configured gap |
| Active Task membership | Show truthful current activity without inventing attribution | New path-only provider join supplies Task Running | Finding 1 | gap |
| Missing planning | Missing evidence cannot become healthy emptiness | Out-of-plan diagnostics relocated to details; Active can hide their Wave | Finding 2; prior diagnostic test | gap |
| One authority | Shared inventories and one window-local surface owner | One Podium Session/roadmap reader and one root registry; existing process-activity read reused | Negative searches | pass for ownership; new attribution semantics need correction |
| Full Task proof | Shared actions/display path, edit round trip, external trials and budgets | Still outstanding | Shared DTO/source and accepted scope | gap |

## Review and configured observations

Read the Task, accepted design including its integration amendment, current slice,
human-demo and Task-view notes. The complete target retains fresh scoped entry,
independent checkout/terminal layouts, ordinary shells, exact native attachment,
and optional A/D presentation. It forbids duplicate inventories, hidden human
boundaries, cwd-based attribution and inferred execution success. LOO-284,
directive editing, other destinations/scopes, human-selected external work and
the required budget/trial series remain outstanding.

Starting HEAD: `9b3264efd46d6b505917930aa257f3c9d11bb1eb`.
`lf task diff LOO-291 --json` reached its cap (`truncated: true`), so recovered
the unrestricted tracked patch with Git:
`/tmp/loo291-review10-complete-tracked.patch`, 1,075,121 characters, 203 sections.
Comparison against the preceding full review patch found 177 identical sections
and 26 changed/new sections. Reviewed the changed executable sections and
supplied/untracked proof material separately; no truncated CLI patch is claimed
as complete. The files continued changing during review, including test updates.

Traced RegistryQuery, Podium, projection/navigation, scoped launch, Session
opening/completion, outer/inner layouts and native ownership. Compared the new
Rust/Swift ActivityNode field and fixture assertions, alongside unchanged Session
mirrors. Searches find one `query.sessions`, one `query.roadmap`, one production
root registry, and none of SessionScope, SessionContext, SessionGroup,
SessionRowItem, PodiumConsole, PodiumSurface or `_loadHierarchy`. LOO-284's shared
legal actions/display path remain absent.

Fresh read-only installed-Home CLI receipts are
`/tmp/loo291-review10-current-{roadmap,sessions,activity}.json`. Roadmap generation
at `2026-09-24T04:35:14.903097Z` exposes Infrastructure, Intelligence and Product
for the canonical repository; LOO-291 remains incomplete. Session list returns
four records; top returns zero nodes. This does not establish why activity is
empty or validate the new Task/provider association. No Session was mutated.

A passive desktop probe at `2026-09-24T04:36:16Z` reports AX trusted, no locked
flag, and focused-application lookup error `-25204`; it exposes no matching
Loopflow application through that runner's NSWorkspace enumeration. Receipt:
`/tmp/loo291-review10-observe.log`, source alongside it. No launch, activation,
keyboard input, permission change or user-client action followed. This is not
the earlier runner's missing-permission observation, nor proof of another
application's focus. The human-demo boundary remains untouched.

## Next slice and verification

Resolve the two findings in the active implementation, finish its focused proof,
then update the disposable configured bundle to those exact bytes. Preserve the
existing signed bundle's historical receipts. Run the prepared exact Session-row
and nested-checkout procedure only when the proof runner has usable foreground
access; do not replay a retired Session or replace a user-owned provider.

No production/test files were edited and no tests were run by this review while
the other writer changed them. The prior four-PTY pass remains historical:
the native test file now includes an All-tasks selection and no longer matches
its recorded hash. `git diff --check` passes. This review does not reuse an older
receipt as validation of the current default filter or run a competing build.

Under [review-slice](/Users/jack/.agents/skills/review-slice/SKILL.md), publication
requires “When all applicable `Done when` claims hold”. The new source findings
and configured gaps leave that condition unmet. No publication, landing or Task
completion occurred.
