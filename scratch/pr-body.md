Closing a PR by branch could leave its Task waiting at an old gate, and failed
GitHub or Git cleanup was silently ignored. Task abandonment now cancels Linear
and the store Task, closes its PRs and deletes its branches through shared lower
operations. It resolves issue IDs, retained branches and the current checkout.
Jack Heart requested this composition and the chapter sweep for LOO-355.

Task PR actions and rebase are issue-addressable. The chapter sweep previews open
issues outside current chapters and excludes live/unresolved execution, open PRs
and missing evidence. Apply rechecks membership and reports incomplete effects.
The existing worker-intent columns prevent execution from starting during
cancellation; a retry preserves completed effects and retained history.

| Need | Task action | Composition and owned records |
|---|---|---|
| File | `task create` | Linear issue; local planning snapshot |
| Allocate or recover checkout | `task checkout ISSUE` | Store Task/PR identity + local checkout/branch |
| Start or resume | `task run ISSUE` | Checkout + saved Flow + worker |
| Pause | `task interrupt ISSUE` | Interrupt current provider turn; retain saved cursor |
| Replace workflow | `task restart ISSUE --flow FLOW` | Checkpoint + stop worker + replace Flow + start |
| Hand off direction | `task comment ISSUE --steer TEXT` | Linear direction + durable steer; does not launch work |
| Split dependent work | `task create --run --stack-on ISSUE` | New issue, Task, checkout and PR based on parent |
| Sync with main/parent | `task rebase ISSUE` | Existing integration operation in Task checkout |
| Publish | `task pr ISSUE publish` | Commit/push + GitHub ready PR + Task PR linkage |
| Open review | `task pr ISSUE open` | Push + draft PR + browser; ready PR stays ready |
| Submit | `task pr ISSUE submit` | Prepare + user merge request; no automatic merge |
| Arm | `task pr ISSUE arm` | Prepare + head-specific auto-merge request |
| Land | `task pr ISSUE land` | Arm + watch/repair + record authoritative merge; keep Task open |
| Land and complete | `task pr ISSUE land -c` | Land + store Done + Linear completed |
| Record success | `task complete ISSUE --summary TEXT` | Complete planning work or a Task with settled delivery |
| Continue serial delivery | `task pr ISSUE next [SLUG]` | Retain prior PR + rotate to next branch, carry follow-up |
| Cancel | `task abandon ISSUE` | Linear canceled + store abandoned + PR abandonment + checkout deletion |
| Delete issue | `task delete ISSUE` | Linear trash + local deletion evidence; retain authored checkout and history |
| Recover interrupted execution | `task run ISSUE --reason TEXT` | Retry saved boundary after correcting the blocker |
| Delete checkout | `wt delete BRANCH` | Remote branch + local checkout/branch; retain PR and Task outcomes |

Validation: focused disposable Git, provider/store cancellation, sweep, command
parsing and existing completion checks; formatting and all-target Clippy. Provider
responses are simulated. See the working design for exact commands and limits.

Not ready to declare LOO-355 complete: completion/landing cleanup still needs
worker-settlement coordination, deletion/recovery composition remains to be
reconciled, and configured disposable-Task and chapter cleanup proofs remain
blocked by the installed CLI's missing commands. LOO-309 and LOO-329 were not
canceled. Read-only GitHub checks confirm #1299 and #1318 are already closed.
