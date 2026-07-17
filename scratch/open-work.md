# Open Work — 2026-07-16

Scope: Product-owned PRs #1020, #998, #990, #980, and #974. PR #1026 is an
untracked test-only follow-up from ENG-20's stale runner, not Product work.

## Pass 1: Clear the decks

| Item | Kind | Age | Status | Recommendation | Why |
|---|---|---:|---|---|---|
| #1020 `make-opencode-glm-sse-disconnects` | worktree + PR | <1d | **Merged** at `a0cb0c65f` after clearing tracked scratch | **landed** | Directly advances the Loopflow API trust KR: hollow OpenCode turns become actionable failures and retry/fallback is explicit. |
| #998 `open-interactive-handoffs-in-the-handoff-real-surface-proof` | worktree + PR | <1d | **Merged** at `6e18861e52` | **landed** | Implements the open PRD-2 proof for attaching Claude through a known durable session id. No conflict or superseding work found. |
| #990 `make-wave-chat-fast-durable-wave-chat-durable-delivery` | worktree + PR | <1d | **Merged** at `7f72eaf050` | **landed** | Makes message acceptance atomic with journal durability, a direct slice of open PRD-3. Small, isolated, and not superseded. |
| #980 `recover-abandoned-task-2` | worktree + PR | <1d | **Merged** at `c78534a3da` after the current-main semantic port | **landed** | The old migration/action patch was discarded. The user outcome now composes with main's successor schema, current-attempt resolution, adoption checks, and shared legal-action model. |
| #974 `let-users-drill-from-roadmap` | worktree + PR | <1d | **Merged** at `6b659d92be` after classifying `runs --wave` in the resolver matrix | **landed** | The task/run/trace identity join directly advances open PRD-12. |
| #1026 `route-automatic-ci-fix-s33dce5bd` | stale-runner PR | <1d | Closed with an abandonment note; `jacklionheart` reopened it 2m later and merged it at 00:03 UTC | **keep merged; do not add revert churn** | Product did not adopt the stale runner, but the landed delta is isolated test evidence. Reverting safe tests after the ownership decision was deliberately superseded would create more work without restoring a product invariant. |

GitHub's merge queue landed the Product set in the order #1020, #998, #974,
#980, then #990, with every head tested against the evolving main branch.

## Pass 2: Wave audit

| Wave | Vision progress | Recent activity | Recommendation |
|---|---|---|---|
| Product | Each landed PR maps to an open Product task and a current proof: Wave Chat durability (PRD-3), handoff fidelity (PRD-2), loop reliability (PRD-5), abandoned-Task recovery (PRD-9), and roadmap-to-trace drill-down (PRD-12). | Five independently useful Product PRs merged today after current-main integration and merge-group CI. | **continue; convert landed capability into KR evidence** |

## Execution

- Landed #998, #990, #1020, #974, and #980 through GitHub's merge queue after
  enabling auto-merge on freshly integrated heads.
- Rebased #974 and composed the run-filter flags with main's `runs reconcile`
  command before shipping.
- Rebuilt #980 on main rather than reviving its obsolete migration. Recovery now
  moves PR ownership, direction, and Linear ingress state atomically onto one
  linked waiting successor.
- Closed #1026 with the stale-runner rationale. `jacklionheart` reopened it two
  minutes later and merged it six minutes after that; retain the isolated tests
  and do not spend a revert on a deliberately superseded ownership decision.
- Kept all worktrees and remote branches; no destructive cleanup was authorized.
