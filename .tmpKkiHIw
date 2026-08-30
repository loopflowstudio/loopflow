# v0.12.14

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.14 makes PR landing a completion lifecycle instead of an auto-merge request. `lf pr land` now owns an exact PR head through CI repair, confirmed merge, and Task settlement, preventing completion or branch rotation from racing ahead of GitHub. Upgrade when automated shipping needs to recover from CI failures and finish against authoritative merge state; use `lf pr arm` when request-and-return behavior is still the right fit.

## Finish only after the PR merges

Landing now follows the PR until GitHub reports a merge or an actionable blocker. Task state changes happen after that confirmation, so the repository and Loopflow no longer disagree about whether work has shipped.

- `lf pr land` watches the exact PR head through merge instead of returning after it requests auto-merge.
- Task completion and serial PR rotation occur only after GitHub confirms the merge.
- If the head changes during landing, Loopflow re-arms the new head rather than treating the earlier request as sufficient.
- `lf pr land -c` completes the owning Task after the watched merge succeeds.

## Repair CI under one durable owner

CI recovery is now part of the landing lifecycle itself. Persisted landing generations and incident records let supervision resume safely after interruption without launching competing repair paths.

- A failed head can launch one bounded `ci-fix` repair for that failure identity.
- Fenced supervisor claims, heartbeats, and Home daemon recovery preserve ownership across process restarts.
- The Task runner's separate CI repair flow has been removed, leaving landing responsible for repair and post-merge settlement.

## Operational notes

- `lf pr land` is now a watched command. Use `lf pr arm` for the previous one-shot behavior that requests exact-head auto-merge and returns.
- Submit, arm, and land preparation collapse authored history into one tree-identical commit before publishing. Expect the prepared PR head and authored commit history to change while the resulting tree remains the same.