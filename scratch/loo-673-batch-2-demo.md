# LOO-673 batch 2 demo: drag, abandon, unassign (2026-11-02)

Branch `jack/loo-673-desktop-polish-fix-jacks-running-feedback` at `cf8000104a`, PR #1791.
Page: `scratch/demo.html`. Intake and audit: `scratch/jack-loo-673-desktop-polish-fix-jacks-running-feedback.md`. Remaining duplicate derivations: LOO-696.

## Proven on the real store (CLI, no app)

Branch `target/debug/lf` vs installed `lf 0.15.1`, read-only against Jack's shared store:

- Repository default selection (the sidebar's Working list) on the installed release: LOO-672, 673, 515, **312**, **330**, **558**, 691, 705. LOO-312, LOO-330 and LOO-558 have no owner.
- Same selection on the branch: LOO-672, 673, 515, 691, 705. The three unowned Tasks are gone; they sit in Backlog.
- `state:working` and `state:autonomous` resolve on the branch (Autonomous: LOO-500, 620, 693); the installed release resolves neither.
- `lf wave show product`: branch groups WORKING 3 / AUTONOMOUS 1 / BACKLOG 33; installed prints NOW / WAITING / NOT STARTED / LATER.

## Not yet observed by Jack

Drag (item 13) and abandon (item 14) are proven by Swift tests only (`draggedRowLandsOnRelease`, `droppedRowMovesBeforeTheWrite`, `abandonLeavesTheSidebarAtOnce`). On 2026-11-02 Jack allowed agents to start Dev apps; this branch's Dev app was built and launched (`scripts/loopflow-dev.py run`, exit 0) replacing the Oct 28 build. Store frontier checked: installed lf 0.15.1 ≥ branch release v0.15.0, no migration in the diff.

## Jack's observations

(pending)

## Next action

Jack tries the three runbook steps in the Dev build; failures go back to LOO-673 as batch 2 rework, acceptance lets #1791 land (CI on the PR shows mostly skipped jobs; recheck before landing).
