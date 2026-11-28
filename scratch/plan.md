# LOO-1125 plan: `realign` stops editing code; code fixes go back through `implement` and `concept-review`

## Direction

Jack Heart replied to `scratch/explore.html`:

> `(1) scratch/explore.html § Q1 · Vision: a (2) scratch/explore.html § Q2 · Docs-only mismatches in build: b (3) scratch/explore.html § Q3 · Refresh and standalone realign: a`

Those options, from the page:

- Q1 a: "Plan-only realign everywhere (prompt, goldens, docs)". No loop-runner
  guard and no commit stamp on review pages in this Task.
- Q2 b: "Allow realign to edit docs inline (only code/tests force iterate)".
- Q3 a: "Update refresh.yaml, AGENTS.md, and docs/orchestration.md to plan-only too".

`scratch/design.md` restates this and is folded in here; nothing in it differs.

## Outcome

In a `build` run, code that `concept-review` flagged is changed by `implement`
and reviewed again before the Graph reaches `compress`. `scratch/concept-review.html`
at `ship-review` describes the code that ships. Today `realign` patches "small,
clear mismatches" itself and returns `advance`, so those edits are never
reviewed and the page still shows "No" rows for code already fixed (runs
`310ac1`, `6db832`, `MTG-497`, from the Task brief).

## The rule

One rule, the same in `build`, `plan`, `refresh` and standalone:

- `realign` writes `scratch/plan.md`, `scratch/questions.md`, Wave memory when
  that is its step's job, and documentation. It never edits a code or test file.
- An in-scope finding that needs a code or test change, however small, is
  written into the plan as the next pass.
- Deciding a loop: `iterate` when the plan holds any in-scope code or test
  change, naming the pass; `advance` when none remains (every row "Yes", fixes
  that were plan or docs only, separate outcomes under `## Deferred findings`);
  `blocked` on a design choice that can't be undone once shipped, as today.

Source of truth is the builtin skill text,
`rust/loopflow-definitions/src/builtins/task/skill/realign.md`. Everything
else repeats or snapshots it.

## Findings behind the plan

- **The loop already does the right thing on `iterate`.** `build.yaml`'s outer
  loop reruns `sync → implement → concept-review → realign`. `implement.md`
  ("When deciding a loop") iterates "while the plan lists a pass not yet
  built", so `realign` has to write the fixes as a named pass, not a loose
  list. `concept-review.md` reviews "the area with the whole change in it
  against its base, not only the last pass" and rewrites
  `scratch/concept-review.page.html`, so a second run produces a fresh page
  with no skill change. No Graph or runner change is needed.
- **Nothing caps loop passes** (no pass limit in
  `rust/loopflow-execution/src/graph`). A fresh review can always find
  something, so `iterate` on any code finding could spin. What stops it is
  already in the text and stays: only findings "the plan has not answered" get
  a place, the plan records what happened to each finding "by substance", and
  a separate outcome is deferred, not built. The rewritten `## Deciding a loop`
  says an already-answered finding does not reopen the loop. The second-pass
  `advance` is part of the demo for this reason.
- **`refresh` has one builtin caller with no `implement` after it:**
  `optimize.yaml` ends `compress → graph: refresh → measure`. There, a code
  mismatch `realign` sees after `sync` stays in `scratch/plan.md` as remaining
  work and nothing builds it. Q3 a chose this; `cmd: sync` owns merge
  conflicts, so what is left for `realign` after a sync is mostly plan drift.
- **Siblings.** `plan.yaml` runs `realign` before any code exists; the rule
  costs it nothing. `implement.md`, `ship-review.md` and `concept-review.md`
  mention `realign` only as the writer of the plan and `## Deferred findings`;
  no wording there says it fixes code. The repository's own `.lf/skills/` has
  no `realign.md` override.
- **The eval already agrees.** `scripts/evals/instruction_levels.json`
  `standalone_realign` allows `inspect_work`, `write_scratch`, `reply`, and not
  `implement`.
- **The refresh description appears nowhere else** (no DTO fixture, doc or
  test quotes "reconcile the plan and code"), so changing it touches one line.
  `compiled.rs` tests check `refresh`'s steps, not its description.

## Choices made for review

- **"Docs" means documentation files, by path.** A comment inside a source or
  test file is a code edit and goes through `implement`. Q2 grouped
  "docs-only or comment-only" in its question, and its chosen answer says
  "only code/tests force iterate"; read by file, the rule has no judgment call
  for an agent to stretch ("it's only a comment" is how inline edits came
  back). Passed over: letting `realign` edit comments in source files.
- **A docs fix made inline leaves its "No" row on the page.** The plan's
  per-finding record says it was fixed in docs, and `ship-review` reads the
  plan beside the page. Passed over: iterating for docs too (Q2 a), which Q2 b
  declined.
- **One eval case is added** (below). Passed over: leaving the loop case
  uncovered by the eval.

## What changes

One pass.

1. `rust/loopflow-definitions/src/builtins/task/skill/realign.md`
   - Frontmatter `produces:` no longer promises an implementation that agrees:
     a plan that matches the code and names the code changes still owed.
   - Opening line and "Is the plan still right?": the sentence "Correct small,
     clear mismatches in code, tests and docs yourself; leave a substantial
     code change as remaining work." becomes: fix docs here; any code or test
     change is remaining work, whatever its size.
   - `## After a review`: "Small, clear mismatch: fix it here, in plan, code,
     tests or docs." splits into a plan-or-docs mismatch (fixed here) and a
     code-or-test mismatch (joins the next pass with rework and missing
     promised behavior).
   - `## Deciding a loop`: the three verdicts as in "The rule", including that
     a finding the plan already answered does not reopen the loop, and that
     the pass is named so `implement` builds it and `concept-review` reads the
     result.
   - Closing lines: "It never adds behavior the plan didn't have" gains the
     plain statement that it never edits code or tests.
   - Keep the skill short; this replaces sentences, it does not add a section.
2. `tests/goldens/builtin_realign.md` regenerated with
   `uv run python tests/goldens/update_goldens.py` (check no other golden moves).
3. `rust/loopflow-definitions/src/builtins/task/graph/refresh.yaml`
   description: integrate upstream changes, then bring the plan up to date.
   `build.yaml`'s comment on `realign` already reads correctly; align
   `docs/orchestration.md:154` with it if the wording drifts.
4. `docs/orchestration.md:172-177`: `realign` takes the findings into
   `scratch/plan.md`, fixes docs inline, defers separate outcomes, and iterates
   back to `sync` for any code or test change so `concept-review` reads the
   final code.
5. `AGENTS.md:285-286` and `:410`: `lf realign` updates the plan (not "plan and
   code"); line 453 already says "reconcile the plan".
6. `.lf/qa/skills.md:55-57`: the `realign`-alone scenario expects the small
   code mismatch queued in the plan with `iterate` and no code diff.
7. `scripts/evals/instruction_levels.json`: one case, `realign` deciding
   `build`'s loop over a review with a small code mismatch; required
   `write_scratch`; allowed `inspect_work`, `write_scratch`, `reply`. Its
   parametrized test in `python/tests/test_instruction_levels.py` picks it up.

## What goes

- The inline-code-fix permission in `realign.md` (three sentences: body,
  `## After a review`, `## Deciding a loop`) and its echoes in
  `docs/orchestration.md`, `refresh.yaml`, `AGENTS.md` and `.lf/qa/skills.md`.
- `scratch/design.md`, superseded by this file.

Stays: `realign`'s triage of findings (defer, adopt a reversible choice marked
for review, block on an irreversible one), `## Deferred findings` for
`ship-review`, the "outside a loop there is no `blocked`" paragraph, Wave
memory curation, and docs fixes inline. Release notes under `release/` are
shipped records and are not edited.

## Exclusions

- No loop-runner guard against a decider that edited tracked files, and no
  reviewed-commit stamp on compiled pages (explore's Vision C). Enforcement is
  the prompt.
- No pass cap on loops.
- No change to `concept-review`, `implement`, `ship-review` or any Graph's steps.

## Acceptance check

Headless, no model:

```bash
uv run python tests/goldens/update_goldens.py && git diff --stat tests/goldens   # only builtin_realign.md
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') cargo test -p loopflow --test golden_prompt < /dev/null
uv run pytest python/tests/test_instruction_levels.py
git grep -n "mismatches inline\|plan and code\|in plan, code, tests" -- ':!release' ':!scratch' ':!.lf/wave'   # no hits
```

## Demo

With the branch build of `lf`, in a fixture repository holding a built change,
a `scratch/plan.md`, and a hand-written `scratch/concept-review.html` with one
small code mismatch, one docs mismatch and one separate outcome (the setup in
`.lf/qa/skills.md`):

```bash
lf -b -a claude:haiku realign
git status --short     # scratch/ and the doc file; no code or test file
```

`scratch/plan.md` names the code mismatch as the next pass and lists the
separate outcome under `## Deferred findings`. Run as the decider in the
outer-loop fixture Graph (about ten minutes on haiku), `realign` returns
`iterate` on pass 1; after `implement` builds the fix and `concept-review`
rewrites the page, pass 2 returns `advance`.

## Done when

- A `build` run whose review finds a small code mismatch goes around the loop
  once more, and the page `ship-review` opens was written after the last code
  change.
- A review with nothing in scope to change in code advances with no extra pass.
- `lf realign` run alone, or inside `refresh`, leaves code and test files
  untouched.

## Security and privacy

Prompt and docs text only; no auth, credentials, telemetry or deployment
surface. `lf security-review` is not needed.
