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
and reviewed again before the Graph reaches `compress`. The page that
`ship-review` opens reflects these fixes. Previously `realign` patched "small,
clear mismatches" itself and returned `advance`, so those edits skipped review
and the page still showed "No" rows for code already fixed (runs
`310ac1`, `6db832`, `MTG-497`, from the Task brief).

## The rule

One rule, the same in `build`, `plan`, `refresh` and standalone:

- `realign` writes `scratch/plan.md`, `scratch/questions.md`, Wave memory when
  that is its step's job, and user documentation (`docs/`, `README.md`). It
  never edits code, tests, builtin skills, goldens, `AGENTS.md` or `.lf/qa/`.
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
  `implement`. Its action catalog (`instruction_levels.py`) has no docs-edit
  action, so inline docs fixes are outside what the eval models.
- **The refresh description appears nowhere else** (no DTO fixture, doc or
  test quotes "reconcile the plan and code"), so changing it touches one line.
  `compiled.rs` tests check `refresh`'s steps, not its description.

## Choices made for review

- **Inline docs narrow to user documentation: `docs/` and `README.md` files.**
  Q2 b stands ("only code/tests force iterate"), but concept-review showed
  "documentation files, by path" doesn't separate Markdown that carries
  behavior: builtin skills compiled into `lf`, goldens, `AGENTS.md`,
  `.lf/qa/` playbooks. Those, and comments in source or test files, are
  code edits and go through `implement`. Passed over: concept-review's
  recommended "scratch only, docs iterate too" (its Q1 A), because it
  reverses Jack's Q2 b; it is in `scratch/questions.md` for him.
- **A docs fix made inline leaves its "No" row on the page.** The plan's
  per-finding record says it was fixed in docs, and `ship-review` reads the
  plan beside the page. Passed over: iterating for docs too (Q2 a), which Q2 b
  declined.
- **The runner's loop criteria defer to the skill's own.** `DECISION_CRITERIA`
  (`rust/loopflow-execution/src/graph/output.rs:11-19`) is appended after
  every deciding step's prompt and says iterate only on "a substantial next
  pass" and block "when passes shrink into small increments"; it lands last,
  after `realign`'s rule to iterate on a small code fix. It gains one line:
  when the skill says how it decides a loop, its rule wins; and `blocked`
  means passes repeating without converging, not one targeted fix pass.
  Defaults stay for skills with no loop section. Passed over: hardcoding
  skill names in `output.rs`, and a separate Task (concept-review's Q2 B),
  since without this the demo's pass-1 `iterate` contradicts the prompt.
- **`realign`'s own shape review runs only when no review just ran.**
  `realign.md:28-31` ("Read the work once as a reviewer…") would, under the
  new rule, turn a cleanup it spots after an all-"Yes" page into a pass right
  before `compress`, which exists to simplify. Reconciling a
  `concept-review`, it triages that page's findings only; standalone,
  `refresh` and post-slice runs keep the shape look. Passed over: deleting
  the paragraph.
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
   - The "Read the work once as a reviewer" paragraph applies when no
     `concept-review` page is being reconciled; with one, triage its findings.
   - Keep the skill short; this replaces sentences, it does not add a section.
2. `tests/goldens/builtin_realign.md` regenerated with
   `uv run python tests/goldens/update_goldens.py` (check no other golden moves).
3. `rust/loopflow-definitions/src/builtins/task/graph/refresh.yaml`
   description: integrate upstream changes, then bring the plan up to date.
   `build.yaml`'s comment on `realign` already reads correctly.
4. `rust/loopflow-execution/src/graph/output.rs` `DECISION_CRITERIA`: the
   skill's own loop rule wins; `blocked` is for passes that repeat without
   converging. Check `cargo test -p loopflow-execution --lib graph::output`
   and any golden that embeds the suffix.
5. `docs/orchestration.md`: updated in `write-docs` (lines 154 and 172–197);
   keep in sync if wording changes during implementation.
6. `AGENTS.md:285-286` and `:410`: `lf realign` updates the plan (not "plan and
   code"); `:453` "reconcile the plan; test only a repair or a real doubt"
   drops "a repair".
7. `.lf/qa/skills.md:55-57`: the `realign`-alone scenario expects the small
   code mismatch queued in the plan with `iterate` and no code diff.
8. `scripts/evals/instruction_levels.json`: one case, `realign` deciding
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

## Concept review (commit `6c6f04ea29`), what happened to each finding

- Runner `DECISION_CRITERIA` contradicts iterating on a small fix: adopted
  into the pass (choice above, item 4).
- `realign`'s own shape review can reopen the loop after an all-"Yes" page:
  adopted (choice above, item 1).
- "Docs by path" is fuzzy: narrowed to `docs/` and `README.md`; the stricter
  option is in `scratch/questions.md`.
- `AGENTS.md:453` still implies repairs: added to item 6.
- `optimize.yaml` has no `implement` after `refresh`: deferred, below.

## Deferred findings

- `optimize.yaml` ends `compress → graph: refresh → measure`; a code mismatch
  `realign` writes into the plan during `refresh` has no `implement` to build
  it. Fixing it means an `implement` step (or branch) after `refresh` in
  `optimize.yaml`; worth it only if optimize runs hit post-sync drift.

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
git status --short     # scratch/ and the docs/ file; no code, test or skill file
```

`scratch/plan.md` names the code mismatch as the next pass and lists the
separate outcome under `## Deferred findings`. Run as the decider in the
outer-loop fixture Graph (about ten minutes on haiku), `realign` returns
`iterate` on pass 1; after `implement` builds the fix and `concept-review`
rewrites the page, pass 2 returns `advance`.

## Done when

- A `build` run whose review finds a small code mismatch goes around the loop
  once more and refreshes the review page after `implement` makes the fix.
- A review with nothing in scope to change in code advances with no extra pass.
- `lf realign` run alone, or inside `refresh`, leaves code and test files
  untouched.

## Security and privacy

Prompt and docs text only; no auth, credentials, telemetry or deployment
surface. `lf security-review` is not needed.
