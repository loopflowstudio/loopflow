# LOO-433: installed measurement of superseded-artifact pruning

Status: plan, 2026-10-27. Implementation already shipped; only the
installed measurement and its durable record remain.

## Problem

Every `lf self install` left `~/.lf/bin/lf-<sha256>` binaries and
`~/.lf-machine/install/artifacts/{published,development}-<sha256>` bundles
behind. PR #1501 (`812d8cc55c`) prunes them when a switch settles. The Task's
last acceptance item is a before/after measurement on the first installed
release that contains it, recorded durably.

## Established facts

- #1501 merged; `812d8cc55c` is an ancestor of `v0.14.18`, the installed
  `lf` (Jack Heart's steer, re-verified with `git merge-base --is-ancestor`).
- The named destination, `docs/reviews/storage-footprint.md`, no longer exists.
  Jack's PR #1592 (`ecbedb4fd1`, 2026-10-18) deleted `docs/reviews/` on purpose:
  "Git retains the removed evidence." Recreating it would reverse that decision.

## Measurement (read-only, 2026-10-27 ~08:30 local, v0.14.18 installed)

| | Before (LOO-390, 2026-10-07) | After (v0.14.18) |
|---|---|---|
| `~/.lf/bin` content-addressed `lf-*` | 168 binaries | 4 |
| `~/.lf/bin` retired `lfd-<sha>` | (included above) | 0 |
| `~/.lf/bin` total `du` | not measured as a whole | 379 MiB |
| `install/artifacts` bundles | 63 | 1 |
| `install/artifacts` total `du` | not measured as a whole | 309 MiB |
| #1501 projection of removable | 161 bins / 6.06 GiB, 62 bundles / 5.32 GiB | — |

Every retained item is justified by the acceptance rule:

- `lf-1307…` is the active selection and published fallback (`active.json`).
- `lf-1bde…`, `lf-2988…`, `lf-d995…` are executables of live processes
  (6, 82 and 8 processes by `ps -axo comm`).
- The one bundle, `published-eef4…`, is the active set.
- Hand-named entries stay untouched: `lfd` and `lf.stale-0.0.0-2026-02-08`
  (Feb 8) and two `hotfix-<sha>/` directories (Jul 17). Unknown ownership is
  not permission to delete — working as specified.
- `retained_published_sets` lists a second set (`published-59a5…`) whose only
  artifact is the same CLI; no bundle is missing for rollback.

Gaps, stated rather than filled: the historical "before" has no whole-directory
`du`, and APFS clone sharing is unmeasured in both columns. No host install or
promotion was run to manufacture a fresh before/after; the brief limits
installation proofs to disposable accounts/containers.

## Decision

Record the measurement as a Task comment on LOO-433 (durable, attributable,
reaches Linear) instead of reviving `docs/reviews/`. No PR: there is no
authorized documentation file to change. Assumption logged in
`scratch/questions.md`.

## Remaining work

1. Done 2026-10-27: measurement posted as a LOO-433 comment
   (progress `e3cd451a22b34ef5856355e115319506`).
2. Complete the Task when the Workflow reaches `end`: all acceptance items are
   delivered (#1501 code, this measurement). No release, rollout or install is
   needed; the root Wave operator's shared release/install is untouched.

Exclusions: no code changes, no builds or test suites (machine load, recent
ENOSPC), no edits to the live machine directories, no process kills,
capture/history retention and other LOO-390 out-of-scope stores.

## Done when

- LOO-433 carries a comment with the after counts/bytes, the before baseline
  and its explicit gaps, and why each remaining artifact was kept.
- The Task is complete with no PR.

Check: `ls ~/.lf/bin`, `ls install/artifacts`, `du -sh`, `active.json`, `ps` — read-only — matched acceptance (4 bins / 1 bundle, all retained items justified).

Concept review (2026-10-27): no change. The model already maps 1:1 —
"retained" means selectable again or running, and unknown names stay. The only
choice the work raised, where the measurement lives, is settled by Jack's
#1592 deletion of `docs/reviews/` plus his steer ("ship only the resulting
authorized documentation change if needed"); no open product question.

Gate (2026-10-27): branch has no diff vs main (0 commits, only scratch); no code, so no build/tests/lints run. Re-check read-only: 4 `lf-<sha>`, 0 `lfd-<sha>`, 1 bundle on lf 0.14.18; measurement comment present on LOO-433.
