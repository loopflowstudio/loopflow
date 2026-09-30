Closing a PR by branch could leave its Task waiting at an old gate, and failed
GitHub or Git cleanup was silently ignored. Task abandonment now cancels Linear
and the store Task, closes its PRs and deletes its branches through shared lower
operations. It resolves issue IDs, retained branches and the current checkout.
Jack Heart requested this composition and the chapter sweep for LOO-355.

Task PR actions and sync are issue-addressable. The chapter sweep previews open
issues outside current chapters and excludes live/unresolved execution, open PRs
and missing evidence. Apply rechecks membership and reports incomplete effects.
The existing worker-intent columns prevent execution from starting during
cancellation; a retry preserves completed effects and retained history.

Completing delivery now removes checkout and branches after the Task worker stops
its provider and settles its claim. Dirty work and branch tips beyond the merged
head survive. `task complete` retries cleanup after partial failure; retired empty
successors retain branch identity. Standalone PR landing also cleans up. Bare Task
landing explicitly keeps its open outcome and checkout for continued delivery.
Task deletion cancels unfinished placed work or cleans completed delivery before
trashing the issue, preserving Task/PR/Run history.

| Need | Task action | Composition and owned records |
|---|---|---|
| File | `task create` | Linear issue; local planning snapshot |
| Allocate or recover checkout | `task checkout ISSUE` | Store Task/PR identity + local checkout/branch |
| Start or resume | `task run ISSUE` | Checkout + saved Flow + worker |
| Pause | `task interrupt ISSUE` | Interrupt current provider turn; retain saved cursor |
| Replace workflow | `task restart ISSUE --flow FLOW` | Checkpoint + stop worker + replace Flow + start |
| Hand off direction | `task comment ISSUE --steer TEXT` | Linear direction + durable steer; does not launch work |
| Split dependent work | `task create --run --stack-on ISSUE` | New issue, Task, checkout and PR based on parent |
| Sync with main/parent | `task sync ISSUE` | Existing integration operation in Task checkout |
| Publish | `task pr ISSUE publish` | Commit/push + GitHub ready PR + Task PR linkage |
| Open review | `task pr ISSUE open` | Push + draft PR + browser; ready PR stays ready |
| Submit | `task pr ISSUE submit` | Prepare + user merge request; no automatic merge |
| Arm | `task pr ISSUE arm` | Prepare + head-specific auto-merge request |
| Land | `task pr ISSUE land` | Arm + watch/repair + record authoritative merge; keep Task open |
| Land and complete | `task pr ISSUE land -c` | Land + store Done + Linear completed + cleanup after worker settlement |
| Record success | `task complete ISSUE --summary TEXT` | Complete settled delivery + cleanup; retry incomplete cleanup by issue ID |
| Continue serial delivery | `task pr ISSUE next [SLUG]` | Retain prior PR + rotate to next branch, carry follow-up |
| Cancel | `task abandon ISSUE` | Linear canceled + store abandoned + PR abandonment + checkout deletion |
| Delete issue | `task delete ISSUE` | Cancel unfinished placed work or clean completed delivery, then Linear trash; retain history |
| Recover interrupted execution | `task run ISSUE --reason TEXT` | Retry saved boundary after correcting the blocker |
| Delete checkout | `wt delete BRANCH` | Remote branch + local checkout/branch; retain PR and Task outcomes |

Validation: the final compression passed 16 focused worker-settlement, completion,
cancellation and sweep tests. Earlier 15 focused completion/deletion tests passed, plus composed
cancellation/deletion, sweep and parser proofs, and the empty-successor landing
regression. Formatting, all-target Clippy and diff checks passed. Real disposable Git repositories/remotes and simulated provider
responses were used; no live cancellation is claimed. Review preserved empty
successor history for retries and moved completion writeback to the owning repo
after discovering that cleanup removes the old path. Worker retirement now uses one finish path for final Flow steps and Tasks completed
inside a turn. Exact commands, earlier fixture failures and static results are
retained in the working design.

Configured acceptance remains after landing and installation, as Jack Heart's
supervising steer directs: disposable Task abandonment, configured Wave sweep
preview/apply, and LOO-309/LOO-329 cancellation. This pass performs no installed-store
repair, promotion, publication or live cancellation. Earlier read-only GitHub
checks found #1299 and #1318 already closed; that is not Task cancellation proof.
