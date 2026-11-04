# LOO-728 — integration assumptions

November 4, 2026: `lf sync --plan origin/jack-heart/write-docs-first` refuses
an explicit target for a stacked Task because sync selects the stack parent.
Jack Heart explicitly requested merging this source while retaining LOO-498 as
parent. A local Git merge of the pinned source prepared the integration without
changing the Task's parent; Loopflow finalized it in `13e7d5dac8`.
No source branch, checkout or PR was removed.

Inherited LOO-498 questions are dependency context, not work assigned here.
Their complete source is
`da41d2031f3526df7fab53292b936dcccc699d9b:scratch/questions.md`;
`rex-workspace.md` retains unresolved scope and `workspace-demo.md` retains the
configured-path failure and native demo identities. Native Split UI, restoration,
release policy, non-default-server routing and remote following remain the
parent's obligations. Raw API counterevidence grants no scope reduction.
