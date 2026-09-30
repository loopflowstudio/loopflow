# LOO-334: runner recovery and execution integration

This Ask follows decide pass 3. The [working design](resolve-tasks-from-linear-and.md)
sections “Slice review — normal promotion prerequisites” and “This slice — normal
promotion prerequisites” retain the implementation and review evidence.
[Open choices](questions.md) retain the separate relationship-repair Ask.

## Observations in this Ask

- HEAD is `b1719ea6c`; the incoming working-design review addition is uncommitted
  and was left untouched. No implementation edits or checkpoint were made here.
- A fresh bounded `docker info --format '{{.ServerVersion}}'` timed out after
  ten seconds. No container was created and no service recovery was attempted.
  Cleanup of `e6e1f0a11850241a3e010fe6e7acbd7d79054b1514c080e8664fd421ef3311a6`
  remains unconfirmed.
- `git merge-base --is-ancestor d61295196 HEAD` returned 1; last-fetched
  `origin/main` remains `a3820bf7e493b7d533a45677b7945020adf80721`.
  This establishes local integration state, not current remote availability.
- `normal_promotion_preserves_pending_task_review` ends after post-promotion
  status and Session-open identity/cwd assertions. It does not complete the review,
  execute the second worker, poison inherited runtime selection, or assert its
  digest. A runner recovery alone cannot satisfy acceptance cases 6 and 15.

## Initial decisions requested

Jack Heart was asked in this conversation to choose runner recovery: restore Docker,
authorize shared service recovery, or identify an alternative isolated runner.
Shared service restart can interrupt unrelated containers; no restart is authorized
by the existing disposable-proof instruction alone.

Jack was also asked whether the next pass should finish the two-worker fixture
independently while deferring ownership changes, or wait for an integrated LOO-298
revision. No integration readiness or new ownership rule is inferred.

## Jack Heart's feedback — 2026-09-30

Jack's supervising session relayed his answer at his request in this conversation:
Docker is responding again, no restart is needed, and the identified stranded
proof container should be removed. Jack directed stacking LOO-334 on LOO-298:
“we can rebase 334 onto 298 and have it land on to 298 then.”

Accepted next steps for the waiting caller:

1. Preserve current work, then use `lf rebase` to rebase this branch onto
   `jack-heart/data-model-one-table-per` (PR #1296). Resolve conflicts toward
   LOO-298's execution model; do not edit its checkout or invent a replacement schema.
2. Retarget PR #1354 to merge into that branch. This specific PR mutation is
   authorized by Jack's relayed direction. The full-code publication boundary
   and remaining acceptance obligations are otherwise unchanged.
3. Continue ownership work against the integrated LOO-298 execution owners and
   finish the normal-promotion proof through two actual worker boundaries.

The relay identifies the latest LOO-334 Task comment as the full direction;
that comment was not independently read in this Ask. The supplied conversation
feedback is the authority recorded here. This supersedes waiting for LOO-298 to
reach main; it does not claim LOO-298 is finished or accepted.

## Recovery verified — 2026-09-30

Bounded Docker inspection returned server `29.4.0` and four running containers.
Inspection resolved `e6e1f0a11850` to the exact previously recorded proof container
`e6e1f0a11850241a3e010fe6e7acbd7d79054b1514c080e8664fd421ef3311a6`
(`rust:1.89-bookworm`). Authorized removal succeeded; a subsequent all-container
query confirmed that exact container absent. No service restart was performed.
This resolves the runner/cleanup blocker, not the unexecuted promotion proof.

## Next action and evidence

After the authorized stack integration, run the disposable harness. Extend the
same normal-promotion scenario through review completion and the second actual
worker, verifying selected runtime/digest and unchanged invocation under poisoned
PATH/LF_BIN/LF_CONTROL_BIN. A reproduced copied-store conflict is useful failing
evidence, not acceptance.

Dependent ownership/migration changes still require LOO-298's integrated stable
owners. Neither equal IDs/bytes nor timestamps select execution succession.
All acceptance cases 1–15 and the full-code publication boundary remain. The
relationship-repair Ask is unchanged. No provider mutation, host promotion,
publication, Task completion, Session completion or Flow navigation occurred.

Return this feedback with `lf session ready`; Jack's Complete action returns it
to the waiting caller. Rebase, PR retargeting and behavioral proof remain caller
work; this Ask has not performed them or selected Flow navigation.

The readiness command was attempted with the decision, all three note paths,
recovery evidence and next proof. Official `lf session ready` exited 1:
`session "ask_once_59bacee26202ef8cfd9a824bf511249e88095695f9909f5044f221b891686d4e" no longer exists`.
Readiness was therefore not recorded. No replacement Session was created and no
completion or worker restart was attempted. The saved feedback remains available
to the supervising session and caller through these notes and this conversation.
