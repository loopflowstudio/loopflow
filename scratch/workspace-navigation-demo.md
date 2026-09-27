# Workspace navigation demo: candidate preparation blocked

2026-09-27 · LOO-303 · interactive demo

The intended experience is command-palette navigation, Task links opening details,
and recursive folded Flow templates. The installed-app walkthrough did not begin:
the installed release predates these changes, and the available branch candidate
also contains the unfinished parent schema conversion. No user observation or
acceptance of the changed behavior was obtained. This is a preparation failure,
not a successful demonstration.

The [current design](workspace-ux-on-data-model.md) and
[delegated demo boundary](workspace-demo-unblock.md) govern. Invocation attempts,
the orphan room and permanent bind remain outside this bounded demo. Their full
Task obligations remain unchanged.

## Fresh observations

- Clean starting HEAD: `b4385406b7be1884ec4f389aae609b432f8dc730`.
  The diff after executable revision `792a3ce40` contains scratch notes only.
- `/Applications/Loopflow.app` reports version `0.12.22`. The selected installed
  CLI's `lf doctor --json` identifies source revision `5838f53a6`, matching the
  prior LOO-291 installation, and reports matching installation/store selection.
- Selected database:
  `/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea/loopflow.db`.
  Its reported migration frontier is `0.12.22.001_release`.
- A separate SQLite connection opened with `mode=ro` finds
  `task_flow_positions`, no `flow_invocations`, `sessions` or `runs` tables,
  and no `tasks.started_at` column. No candidate was opened against this Home.
- Installed `lf roadmap --help` exposes `--wave`, `--json` and `--all`, but
  no `--task`. Thus pointing the new UI at the installed CLI cannot supply the
  new exact Task destination contract.
- Retained source CLI `target/debug/lf` reports `0.12.22+da64c5ee3`, SHA-256
  `b62355336768f248641fc0b99296b51da7b256198f6ee1252e3ad53276b743d2`.
  The retained Xcode app's bundled CLI reports `0.12.22+502ba4601.dirty`, SHA-256
  `9e3cd762d9b73057fc4c2562f3207e0b6bcef4246c9d27c3455ab87af93bda1f`.
  These identify retained artifacts, not an installed candidate or a newly
  verified artifact set.

Machine-readable observation: `.lf/tmp/workspace-demo/readiness.json`.
Prior local behavioral evidence remains in the
[navigation review](workspace-navigation-review.md),
[template review](workspace-template-review.md) and
[return review](workspace-return-review.md). No tests were repeated, and their
fixture/owned-PTY limits still apply.

## Preparation judgment

The independent source slices are implemented, but their current combined binary
is not independently deployable to the selected schema. `release/README.md`
assigns local candidate activation to the supported build/promotion transaction,
including a disposable Home and migration preflight. The parent specification
additionally requires complete conversion and import before activation. The
[parent assessment](workspace-parent-contracts.md) remains dated evidence of
unfinished conversion; this demo did not reassess the parent's moving source.

Source inspection also found that `may_apply_migrations` permits advancement of
private database paths, and `doctor::run` opens `SqliteStore` before its read-only
inspection. Therefore a validation-only build label alone is insufficient reason
to run this candidate against the selected installed-development database. The
candidate was not used there. This is a source observation, not an attempted
migration or observed data-loss incident.

## Feedback and next useful action

No new participant feedback, design approval or appearance verdict was received.
The delegated instruction to reach a demo remains in force; it supplies no
installed acceptance and does not settle the parent's conversion obligations.

Prepare a compatible, reviewable installed candidate through the supported
delivery workflow before resuming this walkthrough. Two routes remain:

1. Complete and integrate the parent conversion/import, then promote the combined
   candidate with preservation and configured readback proof.
2. If an earlier independent demo is required, assess a delivery cut containing
   the navigation/template changes on the installed model through Loopflow's
   delivery workflow. This is a proposal, not an approved branch rewrite or a
   request for a compatibility reader. Prove the same exact lookup/template
   contracts before activation.

Once a compatible candidate is selected, record its binary/commit and Home,
exercise cold and warm `open 'loopflow://task/LOO-303'`, then guide Jack through
palette navigation and recursive template disclosure. Check retained provider
input, capture live 1440×900 and 1100×800 views, and record Jack's verdict.
Inspection must not start work. Missing proof remains missing until exercised.

No application launch/replacement, candidate Home migration, provider interaction,
screen capture, publication or Task completion occurred. The Session was given
the suggested name “Workspace navigation demo.” This note supplies preparation
feedback to the next step and selects no Flow edge.
