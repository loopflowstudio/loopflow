# LOO-497 — LOO-489 Console source

Jack Heart requested integrating LOO-489, rebuilding the Dashboard prototype on
its Console, and continuing after correcting the prototype query. The accepted
boundary is publication for demo review, without merge or Task completion.
The original integration procedure is preserved at
`16c1b19c92:scratch/native-console-integration.md`.

## Materialized handoff — October 17

The complete verified handoff is preserved in git at
`8d4f2d8ea1430fd41d946d328145ce87e26c7b7b:scratch/history/loo489-console-source-2026-10-17/`.
`refs/loopflow/evidence/loo-497-integrated-console` retains that commit.
Its `MANIFEST.json` lists 108 changed files from LOO-489 at
`ac973064c691581610fe563364ddf1a4f239798c`, their source/base copies and SHA256
hashes verified by a second read. The snapshot also retains navigation, demo,
keyboard and earlier LOO-497 notes. Use `git show COMMIT:PATH` to read them.
The integrated production owners now replace these working-tree copies;
`navigation-console-contract.md` retains the accepted navigation contract.

At capture, LOO-489 pursue `72927d43-5e96-48ac-b1a9-134b4836a708` had completed at
demo, with no live Flow. The snapshot was ahead of committed ancestry. It supplied
integration source, not passing combined evidence or missing product permission.

LOO-497 work was checkpointed at `16c1b19c92`; committed ancestry merged at
`6e63af826e`. Three-way reconciliation adopted the Console navigation, retained
workspace renderer, Session preview and provider recovery while preserving the
newer Dashboard/tab contracts. The predecessor Console selection DTO/stream was
removed in favor of LOO-497's shared `PlanningPart.selections` plans. Existing
rounded cards, borders and pane gutters remain in the renderer.

The native Dashboard/editor and portable application persistence now use this
combined Console. Ordinary repository/Wave navigation and stable working slots
remain independent. Current behavior, review repairs, remaining transport evidence
and check results live in `design.md` and `desktop-api-coordination.md`.

## Query correction

Jack Heart rejected `query:product or query:available`, which named invisible
hardcoded fixtures. The study now uses
`(wave:product state:open) or state:available`, with implicit repository scope.
Authored Dashboard references remain valid through the shared API and visible
Dashboard discovery. Prototype code/screenshots in `scratch/query-view` remain
a design study, distinct from native rendering evidence.
