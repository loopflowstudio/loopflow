# LOO-1125 realign routing

## Outcome

In `build`, a code or test fix found by `concept-review` goes into the next
implementation pass. `realign` returns `iterate`, and the outer loop runs
`sync`, `implement`, and `concept-review` again before `compress`. The
review page then covers the corrected code. Runs `310ac1`, `6db832`, and
`MTG-497` reached shipping review with stale rows after inline code fixes.

## Direction

Jack Heart replied to `scratch/explore.html`:

> `(1) scratch/explore.html § Q1 · Vision: a (2) scratch/explore.html § Q2 · Docs-only mismatches in build: b (3) scratch/explore.html § Q3 · Refresh and standalone realign: a`

- Q1 a: "Plan-only realign everywhere (prompt, goldens, docs)".
- Q2 b: "Allow realign to edit docs inline (only code/tests force iterate)".
- Q3 a: "Update refresh.yaml, AGENTS.md, and docs/orchestration.md to plan-only too".

The builtin skill queues implementation fixes of any size in `build`,
`plan`, `refresh`, and standalone runs. It can fix clear user documentation
mismatches under `docs/` and in `README.md` files. Outside a deciding step it
reports remaining work without a loop verdict. The skill and Graph prompts
carry the rule.

## Implementation

- Pass 1 (`7d0554d112`) routed code and test fixes through the plan, restricted
  inline edits to user docs, and limited post-review realign to the review
  findings. The prompt golden, Graph tests, documented commands, eval scorer,
  and CLI previews passed.
- Pass 2 (`41bce8d68`) restored the shared stop and kept skill-specific progress rules. The re-review (`7303cbf672`) found that an unconditional stop conflicts with `optimize-plan` after a reverted trial.
- Pass 3 (`23467d441`) made the shared stop yield to a skill's rule for the case and updated the docs. Graph output tests, the prompt golden, documented commands, and CLI previews passed.

The re-review after Pass 3 (`206eba490b`) found no remaining code, test or
docs mismatch on HEAD: the shared stop now yields to `optimize-plan`'s
reverted-trial rule and to `realign`'s small-fix iterate rule. Its one open
row is the Q2 b choice below. No implementation pass remains.

## Choice for review

The inline docs choice follows Q2 b. The review page can still show a stale
row for an inline docs fix. `realign` records the fix in the plan. The
scratch-only alternative, which would send docs fixes through implementation
and review too, remains in `scratch/questions.md`.

## QA

Done 2026-11-27 on the branch `lf` with haiku workers and a sonnet
`concept-review`; see `scratch/qa.html`. From a seeded review, `realign` fixed
the README, queued the code fix and deferred the separate outcome. In the
build loop, `implement` fixed the code, `concept-review` rewrote the page, and
`realign` returned `iterate` and then `advance`. Standalone and `refresh` left
code and tests untouched. QA tightened the verdict rule in `realign.md` to
"Only a step whose message says it decides a loop returns a verdict". Before
that, two non-deciding runs had named a verdict, one of them in `plan.md`.

## Deferred findings

`optimize.yaml` ends `compress → graph: refresh → measure`. A code mismatch
found during that `refresh` stays in the plan. Add an implementation step or
branch only if actual optimize runs surface post-sync code drift.
