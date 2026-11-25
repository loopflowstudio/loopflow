# LOO-1104: review before compress and qa; `quick` Graph; tighter builtin skills

## Outcome

Status: built (`78bde1d766`), compressed (`c6eab6e222`), QA'd (`73abaa380c`), reviewed (`scratch/concept-review.html`), post-build review findings applied (`02b9cf8f62`), ship-review revisions applied (`4cb640f3f6`: `fix` renamed to `quick`; builtin `ui` removed), and generic design-wording pass applied. Remaining: gate.


A Task on `code`, `feature`, `pursue` or `redesign` runs `compress` and `qa`
once, after the design has settled, instead of once per outer-loop pass. The
measurement behind it (14 days, 5,796 sessions, 154 Tasks): 70.3% of qa spend
($1,063 of $1,512) and 73.7% of compress spend ($387 of $525) were followed by
later implement passes; $735+ (82 qa + 76 compress passes) was invalidated by
the very next concept-review iterating `build`.

One new Graph appears wherever Graphs are listed (`lf list graph`, Desktop's
Start menu and New Task sheet): `quick`, which takes a small, clear change
from brief to merged with no planning or review steps (`merges without asking`).
(Ship-review direction: `fix` was renamed to `quick` because it runs `implement`
rather than `debug`, and `ui` was removed from builtins because UI workflows
belong in repo-specific `.lf/graphs/`.)

Every builtin skill in the loop has one clean owner:
- `concept-review` is read-only everywhere (writes `scratch/concept-review.html`).
- `realign` is the sole owner of reconciling review findings into `scratch/plan.md`.
- `wave-session` is the sole owner of curating Wave `MEMORY.md` (and `wave-operate` stays a read/judge/act Task operator).

## Demo

```text
$ lf graph show build
cmd: sync
implement
concept-review
realign
compress
qa
pr-publish

$ lf list graph        # among the rest
  build   loop(loop(cmd: sync → implement) → concept-review → realign) → compress → qa → pr-publish
  plan    kickoff → concept-review → realign → write-docs
  quick   loop(cmd: sync → implement) → qa → gate → cmd: land --wait-and-fix
```

`code`, `pursue`, `feature` and `redesign` compose over `plan` and `build`
with no edit of their own (`code` compiles to kickoff, concept-review,
realign, write-docs, sync, implement, concept-review, realign, compress, qa,
pr-publish, ship-review, reflect, gate, land, verify).

## The Graphs

Source of truth: `rust/loopflow-definitions/src/builtins/task/graph/*.yaml`.
`build.rs` registers any file dropped there, so `quick.yaml` needs
no registration code.

```yaml
# build.yaml
description: Build the plan pass by pass, review the whole change, then simplify and try it.
steps:
- loop:
    - loop:
        - cmd: sync
        - implement         # decides: another planned pass, or ready for review
    - concept-review        # read-only: writes scratch/concept-review.html
    - realign               # decides: takes the review into the plan; rework, or move on
- compress
- qa
- pr-publish
```

```yaml
# plan.yaml
description: Settle what to build before any code.
steps:
- kickoff
- concept-review            # the plan's premise, read-only
- realign                   # takes the review into the plan
- write-docs
```

```yaml
# quick.yaml
description: Build a small, clear change and land it.
steps:
- loop:
    - cmd: sync
    - implement             # the brief is the plan; decides: more to build, or done
- qa
- gate
- cmd: land --wait-and-fix
```

`pursue.yaml`, `feature.yaml`, `code.yaml`, `redesign.yaml`: no change. Only
the comments in docs that summarize `plan` and `build` move.

### What the mechanics already give

- A loop's last step must be a skill and it decides
  (`rust/loopflow-execution/src/graph/mod.rs:1250`); `implement` and `realign`
  both qualify. A loop may start with `cmd:` (`deep-clean` already does).
- The decider's `iterate` summary becomes the next pass's direction
  (`graph/transitions.rs:84`). So "name the next pass in its summary" is the
  whole handoff from `implement` to the next `implement`, and from `realign`
  back into the inner loop.
- The decision prompt reads `"Decision pass N. Iterating repeats K steps, back to <first step>"`
  (`rust/loopflow/src/lf/commands/graph.rs:1263`). In `build` the first step is
  now `sync`. `GraphOutput::Decision` (`advance` / `iterate` / `blocked`) is
  attached only when `skill.returns.is_some()` (`graph/output.rs:36`).
- Desktop's Start menu (`swift/LoopflowMac/Views/BacklogView.swift:128-145`)
  inspects ops before the first stop and already labels `quick` as
  **`merges without asking`** under `Run without stopping`. `lf task auto off`
  still stops `quick` before `cmd: land --wait-and-fix` (`stopped_landing`,
  `graph.rs:907`).

## Ownership map

### Who writes `scratch/plan.md`

| Step | What it does to the plan |
| --- | --- |
| `kickoff` | writes it |
| `implement` | collapses each finished pass to one line with its commit and any finding that changes the next pass; records a contradiction when a counterexample breaks the plan |
| `concept-review` | never writes it, anywhere |
| `realign` | the only step that turns review findings into plan changes, fixes small mismatches inline, defers separate outcomes under `## Deferred findings`, and regroups remaining passes |
| `compress`, `qa`, `ship-review` | read it |

### Who curates Wave `MEMORY.md`

| Skill | Ownership |
| --- | --- |
| `wave-session` | sole owner of `.lf/wave/<wave>/MEMORY.md` (direction, promoting child-Wave lessons, replacing stale guidance, retiring largest stale sections when `lf context` exceeds its limit). `build.rs:24` composes `wave-session` from `wave_session.md` + `wave_operate.md`, so placing curation in `wave_session.md` keeps it in one section with no duplicate `.lf/wave/` rule. |
| `wave-operate` | unchanged: reads, judges, and moves existing Tasks; never edits Wave files. |
| `realign` | drops `## Wave memory` and its token-limit paragraph; leaves one-line pointer to `lf help wave-session` when a Graph step has write access to Wave memory. |

## The skills

All under `rust/loopflow-definitions/src/builtins/`. Each must still read
correctly in every Graph position it holds (and standalone).

**`task/skill/concept-review.md`** — read-only everywhere.
- Delete the section `## In a Graph, with nobody there` (lines 72–105) and the
  three-outcome report. Move the surviving review guidance (not plan-writing)
  into the main body in a few lines: question the premise, since the plan came
  from the same pipeline; before anything is built, review the whole thing the
  plan builds and whether its first slice is the right one; after it is built,
  review against the base as a first-time reader; show a design choice on the
  page as a recommendation with what was passed over.
- Line 39 ("What this work should do differently goes in the plan, when the
  review is updating one") becomes: every finding goes on the page, marked as
  this work or as a separate outcome.
- Add the closing rule: `scratch/concept-review.html` is the only file it
  writes; it does not edit `scratch/plan.md`, code or docs, and it does not
  decide a loop.
- Frontmatter `produces:` drops "in a Graph, also a plan the next step can
  build from".
- This repository's `.lf/skills/concept-review.md` (`extends: builtin`,
  "Rewrite the whole page") stays valid as is.

**`task/skill/realign.md`** — owns the plan after a review. Runs in three
places: `build` (outer-loop decider), `plan` (after premise review, nothing
built, not a decider), `refresh` (after `cmd: sync` in `queue` and `optimize`,
no fresh review).
- New section for a review page: when `scratch/concept-review.html` has
  findings the plan has not answered, give each one a place:
  - A small, clear mismatch: fix it here, in plan, code, tests or docs.
  - Conceptual rework in the area this work touches, or promised behavior that
    is missing: write it into the plan as the next pass.
  - A separate outcome: list it in the plan under `## Deferred findings` (which
    `ship-review` reads), with what it would take. Don't build it.
  - Record in the plan what happened to each finding by substance (not bare
    numbers), so a later `realign` (e.g. in `refresh`) does not act on the same
    page twice. When a fresh `scratch/concept-review.html` arrives (e.g. after
    `ship-review` iterates back to `build`), replace earlier settled review
    notes while keeping `## Deferred findings` intact.
- Loop decision vs. non-loop honesty:
  - When deciding a loop (`build`): `iterate` only when the plan now holds a
    pass of rework or missing promised behavior, naming that pass in the
    summary; `advance` when what was found is fixed here or deferred; `blocked`
    with the question on a design choice that can't be undone once shipped.
  - Outside a loop (`plan`, `refresh`): no `blocked` verdict exists — record
    any open irreversible choice in `scratch/questions.md` and `scratch/plan.md`,
    report what changed, and finish.
- Widen opening ("After a slice") to "After a slice, a review, or upstream
  changes"; before anything is built, the review is what was learned.
- Keep "Regroup what remains into end-to-end passes": it writes the pass list
  `implement` counts down.
- Delete `## Wave memory` (lines 47–54) and the Wave paragraph of "Keeping
  notes readable" (lines 64–67); leave one line: when the Wave's memory is
  this step's to write, curate it as `lf help wave-session` describes.
  Frontmatter `produces:` drops "Wave memory".

**`wave/skill/wave_session.md`** — absorbs the moved Wave memory curation rules
directly into `## Develop direction` (lines 29–49), with no duplication of the
`.lf/wave/` read-only rule:
- Read the Wave's `MEMORY.md` and its immediate children's; promote lessons that
  apply across the parent, replace stale guidance, remove duplication, and
  curate the selected Wave (not its ancestors).
- Every Wave run pays for `MEMORY.md`: run `lf context`, and when it is over
  its limit retire the largest stale sections until it fits (committing notes
  before removing them), never raising the limit to hide overflow.
- `wave/skill/wave_operate.md`: unchanged.

**`task/skill/implement.md`** — decides the inner loop; language-neutral; no
longer than it is now (110 lines) with the new section in it.
- New short section, conditional on being the deciding step:
  - With `scratch/plan.md`: iterate while the plan lists a pass not yet built,
    and name that pass in the summary; advance once the planned passes are
    built. When a counterexample breaks the plan, note what broke in
    `scratch/plan.md` and return `advance` so review/realign can replan rather
    than iterating onto a broken plan.
  - With no `scratch/plan.md` (the brief is the plan, e.g. `quick`): don't
    write one; judge first whether the brief is one bounded change, and if not
    (or contradicted) return `blocked` before building. Otherwise build it in
    one pass and `advance`. (qa: without "judge first" and "don't write one",
    haiku wrote its own four-pass plan for an oversized brief and iterated
    through OAuth and billing.)
- Pass closeout and context headroom, from a comment on the Task (Jack Heart,
  2026-11-25), word for word:
  > Keep #2037 substantial-pass sizing, and add: do not start another planned
  > pass when context is already heavy; stop cleanly with room to close out.
  >
  > Before returning `iterate`, collapse the finished pass in `scratch/plan.md`
  > to one line with its commit and note any finding that changes the next
  > pass, so `scratch/plan.md` stays lean for downstream steps.

  Both go in the deciding section. The first also bounds "Keep building while
  the next item is the same kind of work" in "Choose the slice": with context
  already heavy, close the pass out and return `iterate` so the next pass
  starts in a fresh conversation. The second replaces the looser "Mark the
  plan's remaining work in place: what this slice built, what it taught, what
  is next" when iterating: one line per finished pass with its commit, plus
  only the finding that changes the next pass.
- "If the codebase uses `@dataclass`, use `@dataclass`" becomes "use the
  constructs the codebase already uses for the job".
- Trim what `LOOPFLOW.md` already says on every launch, keeping the parts
  specific to a slice: "A new type means a new real-world concept"; "Stay in
  scope… `scratch/questions.md`"; the generic half of "Write the tests"; the
  closing paragraph on staying in this checkout and starting no second Task.
  "Finish the slice by deleting" keeps its test-deletion rule.
- Runs unchanged as a non-decider in `optimize` and `.lf/graphs/*`;
  `.lf/skills/implement.md` extends it and needs no edit.

**`task/skill/demo.md`** — unchanged (reverted after builtin `ui.yaml` was removed).

**`task/skill/verify.md`** — identical-tree fast path, placed before "Check
headless first":
- When every shipped file on `main` is byte-for-byte identical to the tree `qa`
  exercised (`git diff <qa head> <merge SHA> -- . ':!scratch'` prints nothing;
  `scratch/` is excluded because `lf land` clears added scratch files before
  merging), post-merge CI for `<merge SHA>` is green, and no claim depends on
  something that only exists after merge (a release install, a deployment,
  production state), the claims were already checked on these exact files:
  verify from the `qa` and `gate` records and the CI run, cite them, and don't
  rebuild `main`.
- Any of the three missing (or any diff outside `scratch/`, including code
  edits by `gate`/`land` or synced upstream commits) means the full check, as
  today.

**Generic wording & `ship-review`**
- `kickoff.md:26`: "Linear's API has no conditional assignment…" becomes "the
  tracker's API has no conditional assignment…".
- `advance.md:22`: "No Task, Wave or Linear setup" becomes "No Task, Wave or
  tracker setup". Its `description:` ("the current Loopflow work") stays.
- `write-docs.md` step 4: `lf open --file <path>` (repeat `--file`), which
  shows files in an attached Files view and never launches anything; with none
  attached, report the paths (`open/files.rs:18` accepts host `auto` and prints
  "No Desktop Files view attached; nothing opened."). Frontmatter `produces:`
  ends "with their paths shared", no "Desktop".
- `ship-review.md` gather list: deferred findings come from `## Deferred
  findings` in `scratch/plan.md` (written by `realign`), with
  `scratch/concept-review.html` as the page behind them. One line changes.

## Everything else that names the old order

Tests (expected values change; no production Rust changes):
- `rust/loopflow-execution/src/graph/compiled.rs:603` and `:685`
  (`pursue_loops_to_a_published_pr`): labels become kickoff, concept-review,
  realign, write-docs, sync, implement, concept-review, realign, compress, qa,
  pr-publish; returns are step 5 → step 4 and step 7 → step 4.
  `steps[4].sources == ["pursue", "build"]` still holds (step 4 is still sync).
- `rust/loopflow-execution/src/graph/mod.rs:1542`: edges become
  `[("implement", 4), ("realign", 4)]`.
- `rust/loopflow/src/lf/commands/graph.rs:1646`: add `quick` to the list with no
  interactive step.
- `rust/loopflow-execution/src/task_graph.rs:759` (concept-review placed in
  `plan` then `build`) holds unchanged.
- `tests/goldens/builtin_implement.md`, `builtin_realign.md` (and any golden
  covering `wave_session` if present): regenerate with
  `uv run python tests/goldens/update_goldens.py`.
- DTO fixtures under `tests/fixtures/dto/` and Swift tests name no step order
  from these Graphs.

Docs:
- `docs/orchestration.md`: updated the `build` listing and prose (`implement` decides inner loop, `realign` decides outer loop, `Decision pass` example says `back to sync`), updated `feature` listing comments, and added `quick` to the Built-in Graphs table with example and note.
- `docs/index.md` and `README.md`: replaced stale `auto` in the Built-in Graphs list with `quick`; updated `docs/index.md` loop example to `sync → implement` inside `concept-review → realign`.
- `docs/surfaces.md`: updated Start menu sample with `quick` (`merges without asking`) under `Run without stopping`, with a note on `merges without asking` and `lf task auto off`.
- `docs/work.md`: replaced "`realign` keeps plan, code, and memory in sync" with `wave-session` curating Wave `MEMORY.md`.
- `swift/README.md:407`: read and kept (still true: `concept-review` runs in both `plan` and `build`).

## Choices made for review

1. **Wave memory curation moves to `wave_session.md`, not `wave_operate.md`.**
   `build.rs` composes `wave-session` by appending `wave_operate.md` to
   `wave_session.md`; `wave_session.md` already owns writing and publishing
   `MEMORY.md` while `wave_operate.md:10` says Wave files are not its to
   change. Passed over: putting `## Wave memory` in `wave_operate.md` as the
   brief drafted, which creates a self-contradiction in `wave-operate` and
   duplicate `MEMORY.md` rules inside `wave-session`.
2. **`implement` broken-plan rule keys off `scratch/plan.md` presence.**
   `implement` cannot see what steps follow its loop in the Graph. With
   `scratch/plan.md` (`build`), it notes what broke and advances to
   review/realign; without `scratch/plan.md` (`quick`), it returns `blocked` so
   broken work never proceeds to `qa → gate → land`. Passed over: letting
   `implement` rewrite the pass list and iterate, or asking it to guess Graph
   topology.
3. **`verify` compares `git diff <qa head> <merge SHA> -- . ':!scratch'`.**
   Comparing `<qa head>` (not `<final PR head>`) catches post-`qa` edits by
   `gate`/`sync`/`land`; comparing `<merge SHA>` (not moving `origin/main`)
   avoids false diffs from later PRs; excluding `scratch/` keeps `lf scratch
   clear` on `land` from breaking the fast path on every PR.
4. **`quick` merges with no interactive step.** Matches the brief and
   ship-review revision; Desktop's Start menu already warns `merges without
   asking`, and `lf task auto off` still stops before `land`. Passed over:
   adding `interactive: verify` at the end.

## Known costs of the new order

- `concept-review` runs before `compress` and `qa`, so it never reads qa's
  findings. Design-level problems qa finds go to `scratch/qa.html`, which
  `ship-review` gathers; in `pursue` (no `ship-review`) they reach a person
  through the PR.
- `sync` runs before each `implement` (in `build` and `quick`), so the final
  pass is not synced before `compress` and `qa`. `gate` and
  `land --wait-and-fix` sync later.
- `quick` has no interactive step, so `lf task run` merges as soon as checks
  pass unless `lf task auto off` is set.

## Left out

- `loop-or-next`, `qa`, `compress`, `gate`, `reflect`, `demo`, `wave_operate` text.
- Builtin `ui` Graph (UI workflows belong in repo-specific `.lf/graphs/`).
- This repository's `.lf/graphs/*` and `.lf/skills/*`.
- Choosing `quick` automatically from a Task's content; a person or Wave
  default picks the Graph.
- `.lf/wave/intelligence/MEMORY.md` (read-only here; its 2026-09-28 note that
  `realign` owns memory curation waits for a Wave session update).

## What goes

- `concept-review.md`: the headless plan-writing mode and three-outcome report.
- `realign.md`: `## Wave memory` and the Wave-memory limit paragraph (moved to
  `wave_session.md`, not copied).
- `implement.md`: the `@dataclass` example and rules `LOOPFLOW.md` repeats.
- The old expected step lists in the three Rust tests above.

Must survive: `concept-review` with a person (the page stands in for code
review; nothing else changes unless asked); `realign` in `refresh`; `implement`
as an ordinary step; `verify`'s three outcomes.

## Acceptance check at gate

```bash
cargo build -p loopflow
# Run from an empty temp repo with LF_* unset so repo .lf/ doesn't shadow builtins:
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') sh -c '
  tmp=$(mktemp -d) && git -C "$tmp" init -q &&
  (cd "$tmp" && "$PWD/target/debug/lf" graph show build &&
   "$PWD/target/debug/lf" graph show code &&
   "$PWD/target/debug/lf" list graph | grep -E "^  quick ")
'
grep -rnE "Linear|Loopflow Desktop|--host desktop" \
  rust/loopflow-definitions/src/builtins/task/skill/{kickoff,advance,write-docs}.md   # no output
uv run python scripts/test.py --reuse-passing \
  --rust-filter 'test(graph::) | binary(golden_prompt) | test(documented_commands::) | test(builtins::)'
```

## Passes

One pass; it is prose, three YAML files, docs, and test expectations.

- Built in one pass (`78bde1d766`), compressed (`c6eab6e222`), and QA'd (`73abaa380c`):
  - Reordered `build.yaml` and `plan.yaml`; added `quick.yaml` (originally `fix.yaml`).
  - Updated `concept-review.md`, `realign.md`, `wave_session.md`, `implement.md` (109 lines after QA tightening), `verify.md`, `kickoff.md`, `advance.md`, `write-docs.md`, `ship-review.md`; regenerated `tests/goldens/builtin_{implement,realign}.md`.
  - Updated Rust graph expectations in `compiled.rs`, `graph/mod.rs` (`edges()` reports relative offsets `1` and `3`, not absolute indices), `commands/graph.rs`, and `task_graph.rs:631` (`write-docs` shifted by +1 after `realign` joined `plan`).
  - QA (`scratch/qa.html`) verified `lf graph show`, `lf list graph`, Desktop `StartFlow` labels, `lf help`, and real Claude haiku decider runs for `build`'s inner loop, bounded brief, read-only `concept-review`, and oversized brief (caught and fixed in `73abaa380c`: no-plan `implement` now blocks oversized briefs before writing any plan or code).
- Post-build review (`02b9cf8f62`): clarified `realign.md` reversible-choice handoff and `verify.md` `<qa head>` pointer.
- Ship-review revision (`4cb640f3f6`): renamed `fix.yaml` to `quick.yaml` ("Build a small, clear change and land it."), removed builtin `ui.yaml` and reverted `demo.md`, and updated tests, docs, and PR #2047 copy.
- Generic "design" wording pass (applied): removed "product" wording from `concept-review.md` (using "the system", "domain concepts", "the concepts people work with", and "design choice"), aligned "product choice/decision" to "design choice/decision" in `realign.md`, `kickoff.md`, and `write-docs.md`, regenerated `tests/goldens/builtin_realign.md` (no `concept-review` golden exists), and updated this plan's prose.

  Steer from a comment on the Task (Jack Heart, 2026-11-25), word for word:

  > Additional wording edit in builtin `concept-review.md` (and update its golden): remove "the product" / "product" wording (currently ~10 occurrences) so `concept-review` fits libraries, engines, compilers, protocols, and research pipelines as well as user-facing apps.
  >
  > Replace with generic framing: e.g. "the system" / "the domain" / "the concepts people work with" / "a design choice" instead of "the product", "product concepts", "product choice", and "what the product should be". Align the matching "product choice/decision" phrases in `realign.md`, `kickoff.md`, and `write-docs.md` to "design choice/decision" too.

## Post-build review findings

Recorded by post-build `concept-review` against base `23ce1a69f9` (`scratch/concept-review.html`), both resolved before `pr-publish`:

1. **Resolved — `realign.md` reversible-choice handoff:** added a bullet under `## After a review` so headless `realign` adopts a recommended reversible design choice into the plan marked as made for review (with what was passed over and why, never as settled), and regenerated `tests/goldens/builtin_realign.md`.
2. **Resolved — `verify.md` `<qa head>` pointer:** clarified that `<qa head>` is the branch's `lf qa:` commit.

## Deferred findings

- **Update `.lf/wave/intelligence/MEMORY.md`:** read-only in a Task checkout; replace the 2026-09-28 entry ("`realign` now owns memory curation") with "`wave-session` owns memory curation" in an intelligence Wave session once LOO-1104 lands.
- **Step-commit `-a` propagation (`scratch/qa.html` #1):** with `-a claude:haiku` and no `default_agent`, `lf commit --step` falls back to `lf <skill>: checkpoint`. Outside LOO-1104's scope; `.lf/qa/skills.md` documents the current behavior.
- **Repo-local UI workflows, `.lf/graphs/pursue-auto.yaml` & automatic Graph selection:** separate outcomes outside this Task.

Done when `lf list graph` shows `quick` beside the reordered `build`
and `plan` (with no builtin `ui` or `fix`); `lf graph show code` runs
`concept-review` and `realign` before `compress` and `qa`; a headless
`concept-review` leaves `scratch/plan.md` untouched; `wave-session` is the
single owner of Wave memory curation; `verify` has the `scratch`-excluded
identical-tree fast path; and the gate check passes.
