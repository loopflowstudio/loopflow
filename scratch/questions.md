# Open assumptions

LOO-298 · 2026-09-29 · Jack Heart's decisions are in
[the contract](data-model-one-table-per.md). Earlier questions and superseded
proposals remain in [committed history](https://github.com/loopflowstudio/loopflow/blob/d6dc8c43b73872b8b6e71b0996468d2432b23102/scratch/questions.md).

- **Usage after bind:** prospective attribution remains the operating assumption.
  Jack leans toward post-hoc attribution but has not selected it. Earlier events
  keep their recorded owner; an active turn's cumulative usage cannot silently
  move or be split without evidence. Bring this choice to concept review.
- **Blocked feedback:** retain `lf flow blocked` as a keyed Ask returning feedback
  under the selected conversation's current driver. It supplies no verdict.
  Jack has not selected a replacement; bring the retained interaction to review.
- **Router removal:** remove it with the decision command as the operator's
  assumption under Jack's autonomous goal. Structured output is the only verdict
  and route writer. No compatibility command remains.
- **Captured events:** use kind `captured`; reservation inserts it before launch.
  Preserve artifact directory keys and receipt strings as immutable evidence,
  with sequence identity for current selection and publication. Unclassified SQL
  goes to immutable `import_evidence`, never a fabricated AgentSession. This is
  an import archive with no runtime reservation or lifecycle writer.
- **Runtime passes:** each taken backward edge starts a child pass; initial
  forward execution stays in the root. Returning Iterate creates a sibling;
  an inner edge creates a nested child. This interpretation of Jack's contract
  remains explicit for final review, with recorded source/canonical/public proofs.
- **Whole-Flow binding:** outside Session bind scope. A taskless Flow and its
  member Sessions cannot acquire incompatible Tasks piecemeal.
- **Wave without a current Project:** automatic creation on Task start remains
  unselected. Report the missing current plan.
- **Exec discovery:** list defaults to 100 rows in the current repository;
  `--all` spans repositories and `--after` uses the prior JSON cursor with the
  same filters. Pagination is not snapshot isolation. Every-process observation
  still needs final disposition for parser/help/version, bootstrap/installation
  and screenshot paths; missing completion is never a successful receipt.
- **Import and proof limits:** original SQL payloads, selectors, ancestry and
  Started remain evidence even without known conversation membership. Imported
  observations cannot settle a native Flow turn. Local retirement cannot prove
  absence of another Home's work; fixtures cannot establish configured accounts,
  rendered Desktop or installed migration. Retain original artifacts until the
  full import obligations pass. Prepare the real-Home procedure without executing
  a branch binary against the installed Home or promoting it.

No additional product decision is needed to continue implementation. The
[remaining work](remaining-work.md) and [import obligations](import-preservation.md)
retain acceptance and preservation requirements.

- **Discovery surface:** use `flow list/show --sessions` for saved FlowSessions,
  keeping the proposal's `sessions` and `inspect` names available as templates.
  Session inventory has an explicit page mode for Desktop; ordinary alphabetical
  inspection remains. These are implementation choices, not attributed decisions.

2026-09-29 · **A launch is an Exec: decided, and part of LOO-298.** Jack Heart
asked "why not have lf flows actually launch skill execs?", said it "makes the
hierarchy way better and clearer", and then directed: "this is part of 298" and
"rearrange the tasks around this -- it's central and we should do the deepest
cuts first." Each Flow step runs as its own `lf` process. The Flow driver keeps
the claim and spawns one child `lf` per step; mechanical steps that are already
lf commands run as child Execs too. No new object: a step is an ordinary Exec.
The operator had filed LOO-335 for this minutes earlier, before Jack's
direction; it is superseded by LOO-298 and awaits Jack's word on deletion.
Implementation choice in [the Exec design](exec-per-step.md): process start/exit
belong only to Exec. Retain captured instructions as a Session history event,
not a second launch lifecycle; capture belongs to the executing child when a
process is known. Imported missing Exec stays unknown. Jack has not reviewed
this implementation choice yet.

2026-09-29 · **Vocabulary: prefer Exec.** Jack Heart: "if a launch is an exec,
we should also try to use the word exec instead of launch (or run etc) where
possible." This supersedes the operator's suggested word "launch" and the
assumed event kind `captured`/`launched`. Names, CLI text, docs and wire fields
that mean one agent start under one lf process say Exec. Where no lf process
exists, as with imported history, the name must not claim one.

2026-09-29 · **After Exec-per-step: simplify attribution and the parent tree.**
Jack Heart: "Make sure that we simplify and clarify attribution and the parent
tree after this." This is a required follow-on inside LOO-298, directly after
the Exec-per-step cut. It is a reduction: name the one place each fact lives,
derive the rest, and delete the copies.

2026-09-29 · **Requirement kept through the simplification.** Jack Heart: "to be
clear i still want being called by an agent process to give you the right
parent-lf process as your lf exec parent." An lf command issued by an agent
records, as its parent Exec, the lf process that is driving that agent. The
provider and shell processes in between are not Execs and are skipped.

## Mechanical child entry · 2026-09-29

Implementation choice under Jack Heart's accepted Exec-per-step direction:
`__flow-step ID VERSION` is a hidden entry for captured boundaries. Internal
children use the invoking driver's executable and inherited store, so a PATH or
selected-installation change cannot silently swap schemas during a Flow. The
existing full Task claim travels as launch context and remains owned by the
driver; no second durable authority is added. Agent conversion is still pending.
