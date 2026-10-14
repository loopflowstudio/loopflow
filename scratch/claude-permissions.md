# Session launch integration

Accepted by Jack Heart, 2026-10-14. The Session launch/API consolidation is published as PR #1553, head `161958efe325350b31838908bb9d1f7d59c89e28`. Queue gate passed before publication. Jack then requested handling conflicts with #1551 and explicitly selected landing #1551 first. #1551 is already in GitHub's merge queue; #1553's watcher was stopped and auto-merge disabled to preserve that order.

## What remains

Jack subsequently authorized stacking on #1551. Merge its branch now, reconcile the overlapping Session APIs, verify the combined behavior, and run the authorized landing-with-wait operation for #1553. The installed spelling is `lf land --wait-and-fix`, equivalent to Jack's requested `lf land --watch`. Landing is authorized after verification; no further confirmation is needed.

## Binding behavior

- `lf resume` and `lf session resume` take no Session ID. Explicit Task/Wave overrides cwd; active Task wins in its checkout; main chooses the latest interactive Session across repository worktrees; an unowned linked checkout chooses its latest local interactive Session. Resume never creates a missing primary.
- `lf connect ID|--task ISSUE|--wave NAME` shares selection/connection; Work connect can prepare a missing primary. `--prepare` launches no provider; JSON controls formatting only. Desktop consumes the same preparation. `session new` changes primary identity; `connect --takeover` preserves conversation identity.
- Fresh and stopped launches use current settings from the selected checkout, including YOLO/Chrome/model, while same-provider resume retains native history/account. Live attachment keeps its running policy. Historical replay uses captured inputs.
- #1551 adds explicit cross-provider continuation: a new native conversation reads the prior conversation log while preserving the Loopflow Session, Work and feedback. Retain the import's retry/pending-input behavior. Do not restore refusal of all provider changes. Never pass an old provider's token/account into the new provider.
- Preserve upstream #1552 Claude stdin and Codex protocol/native-client startup, hooks, large prompts, failed-start recovery and execution boundaries.
- No schema change, compatibility launcher or duplicate policy resolver. Wave-worktree delivery policy remains out of scope.

## Integration cuts

**This slice:** land #1553 after verified integration. #1551 merged at 2026-10-14T18:53:18Z; local merge 1af23b5eff combines it with #1553. Overlap includes config resume choice, headless continuation, native connection, Session open/import, capture metadata and launch fixtures. Delete obsolete resume-model refusal and duplicate per-field launch setup; keep one settings resolver and provider translation.

Acceptance: all 15 launch-contract tests plus #1551 cross-provider continuation/retry coverage; include headless continuation and same-provider current-policy/account retention. Format/clippy as required after code repair. Broader reruns only for changes, failures or specific unresolved integration risks. Update the PR's scope and proof to match the final combined change, then land with CI watching.

Check (2026-10-14, pre-#1551 integration): queue gate passed 2,591 distinct Rust tests (including 15 launch contracts), 508 Swift tests, 75 website tests, formatting/clippy/architecture/inventory/boundary/build checks; Linux-only checks deferred to CI. PR #1553's first failing Linux shard failed downloading its artifact with GitHub DNS ENOTFOUND before executing tests. Integration proof: cargo test -p loopflow --test session_launch_contract (16 passed); focused Task/Flow/provider-switch recovery (1 passed), conversation-log (4 passed), native/import selection (1 passed); cargo check -p loopflow --tests, cargo fmt, cargo clippy --all-targets -- -D warnings and git diff --check passed. Review retained one resolver and verified that imported conversations do not inherit the earlier provider’s native token/account.

The user-requested HTML and full earlier design/coverage evidence are preserved under `/Users/jack/.lf/reports/session-launch-20261014/`; landing may clear scratch.
