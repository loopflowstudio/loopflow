# LOO-1104: review before compress and qa; `ui` and `fix` Graphs; tighter builtin skills

## Outcome

A Task on `code`, `feature`, `pursue` or `redesign` runs `compress` and `qa`
once, after the design has settled, instead of once per outer-loop pass. The
measurement behind it (14 days, 5,796 sessions, 154 Tasks): 70.3% of qa spend
($1,063 of $1,512) and 73.7% of compress spend ($387 of $525) were followed by
later implement passes; $735+ (82 qa + 76 compress passes) was invalidated by
the very next concept-review iterating `build`.

Two new Graphs appear wherever Graphs are listed (`lf list graph`, Desktop's
Start menu and New Task sheet): `ui`, which puts the running UI in front of a
person every pass, and `fix`, which takes a bounded bug from brief to merged
with no planning or review steps.

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
  fix     loop(cmd: sync → implement) → qa → gate → cmd: land --wait-and-fix
  plan    kickoff → concept-review → realign → write-docs
  ui      kickoff → loop(implement → cmd: sync → qa → demo (interactive)) → compress → pr-publish → deliver
```

This output is real: the four YAML files below were written to a temp
repository's `.lf/graphs/` and read with the installed `lf`. All four compile,
and `code`, `pursue`, `feature` and `redesign` compose over them with no edit
of their own (`code` becomes kickoff, concept-review, realign, write-docs,
sync, implement, concept-review, realign, compress, qa, pr-publish,
ship-review, reflect, gate, land, verify).

## The Graphs

Source of truth: `rust/loopflow-definitions/src/builtins/task/graph/*.yaml`.
`build.rs` registers any file dropped there, so `ui.yaml` and `fix.yaml` need
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
# ui.yaml
description: Build a Desktop, mobile or web UI change you try as it takes shape, then deliver it.
steps:
- kickoff
- loop:
    - implement
    - cmd: sync
    - qa
    - interactive: demo     # you use it; iterate goes back to implement with your words
- compress
- pr-publish
- graph: deliver
```

```yaml
# fix.yaml
description: Fix a bounded bug or make a small tweak, and land it without stopping.
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
  (`rust/loopflow-execution/src/graph/mod.rs:1250`); `implement`, `realign`
  and `interactive: demo` all qualify. A loop may start with `cmd:`
  (`deep-clean` already does).
- The decider's `iterate` summary becomes the next pass's direction
  (`graph/transitions.rs:84`). So "name the next pass in its summary" is the
  whole handoff from `implement` to the next `implement`, and from `realign`
  back into the inner loop.
- No autonomous loop may return across an interactive step
  (`validate_interactive_steps`, `graph/mod.rs:609`). `ui`'s loop is decided by
  the interactive step itself, like `code`'s `ship-review` loop, so it passes.
- The decision prompt reads "Decision pass N. Iterating repeats K steps, back
  to <first step>" (`rust/loopflow/src/lf/commands/graph.rs:1263`). In `build`
  the first step is now `sync`; the docs example in `docs/orchestration.md`
  follows whatever the built `lf` prints.

## Who writes `scratch/plan.md`

| Step | What it does to the plan |
| --- | --- |
| kickoff | writes it |
| implement | collapses each finished pass to one line with its commit and the finding that changes the next pass; revises when a counterexample breaks the plan (unchanged) |
| concept-review | never writes it, anywhere |
| realign | the only step that turns review findings into plan changes, and the only one that regroups remaining passes |
| compress, qa, ship-review | read it |

"Single owner" in the brief is read as the last two rows: review findings
reach the plan through `realign` and nowhere else. `implement` still marks its
own progress, because `ui`, `fix` and `optimize` have no `realign` after it.

## The skills

All under `rust/loopflow-definitions/src/builtins/`. Each must still read
correctly where it runs outside these Graphs; the sibling uses are listed.

**`task/skill/concept-review.md`** — read-only everywhere.
- Delete the section "In a Graph, with nobody there" (lines 72–105) and the
  three-outcome report. What survives from it moves into the main body in a few
  lines, because it is review guidance and not plan-writing: question the
  premise, since the plan came from the same pipeline; before anything is
  built, review the whole thing the plan builds and whether its first slice is
  the right one; after it is built, review against the base as a first-time
  reader; a product choice is shown on the page as a recommendation with what
  was passed over.
- Line 39 "What this work should do differently goes in the plan, when the
  review is updating one" becomes: every finding goes on the page, marked as
  this work or as a separate outcome.
- Add the closing rule: the page is the only file it writes; it does not edit
  `scratch/plan.md`, code or docs, and it does not decide a loop.
- Frontmatter `produces:` drops "in a Graph, also a plan the next step can
  build from".
- This repository's `.lf/skills/concept-review.md` (`extends: builtin`,
  "Rewrite the whole page") stays valid as is.

**`task/skill/realign.md`** — owns the plan after a review. Runs in three
places and the text must fit each: `build` (outer-loop decider), `plan` (after
the premise review, nothing built, not a decider), `refresh` (after `cmd:
sync` in `queue` and `optimize`, no fresh review).
- New section for a review page: when `scratch/concept-review.html` has
  findings the plan has not answered, give each one a place.
  - A small, clear mismatch: fix it here, in plan, code, tests or docs.
  - Conceptual rework in the area this work touches, or promised behavior that
    is missing: write it into the plan as the next pass.
  - A separate outcome: list it in the plan under a heading `ship-review`
    reads, with what it would take. Don't build it.
  - Record in the plan what happened to each finding, so a later `realign`
    (the one in `refresh`) does not act on the same page twice.
- When it decides a loop: iterate only when the plan now holds a pass of
  rework or missing promised behavior, naming that pass in the summary;
  advance when what was found is fixed here or deferred. A product choice that
  can't be undone once shipped is `blocked` with the question. Outside a loop
  it reports and finishes.
- "After a slice" opening widens to "after a slice, a review, or upstream
  changes"; before anything is built, the review is what was learned.
- "Regroup what remains into end-to-end passes" stays: it writes the pass list
  `implement` now counts down.
- Delete `## Wave memory` (lines 47–54) and the Wave paragraph of "Keeping
  notes readable" (lines 64–67); leave one line: when the Wave's memory is
  this step's to write, curate it as `lf help wave-operate` describes.
  Frontmatter `produces:` drops "Wave memory".

**`wave/skill/wave_operate.md`** — gains `## Wave memory`, the moved text:
read the Wave's `MEMORY.md` and its immediate children's, promote lessons
that apply across the parent, replace stale guidance, curate the selected Wave
and not its ancestors; `.lf/wave/` is read-only unless the running Graph's
step maintains it, otherwise proposals go in `scratch/`; `lf context` over
its limit means retiring the largest stale sections, never raising the limit.
Line 10 ("its plan, new Tasks and backlog are not this skill's to change")
keeps its meaning: memory curation is conditional on write access.

**`task/skill/implement.md`** — decides the inner loop; language-neutral;
no longer than it is now (110 lines) with the new section in it.
- New short section, conditional on being the deciding step: iterate while the
  plan lists a pass not yet built, and name that pass in the summary; advance
  once the planned passes are built. With no `scratch/plan.md` the brief is
  the plan and its outcome is one pass. When a counterexample breaks the plan,
  don't iterate onto it: advance (or `blocked` when no later step can replan)
  and say what broke.
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
  starts in a fresh conversation (each `iterate` launches the loop's skills
  afresh). The second replaces the looser "Mark the plan's remaining work in
  place: what this slice built, what it taught, what is next" when iterating:
  one line per finished pass with its commit, plus only the finding that
  changes the next pass. This is the job `realign` did between passes.
- "If the codebase uses `@dataclass`, use `@dataclass`" becomes "use the
  constructs the codebase already uses for the job".
- Trim what `LOOPFLOW.md` already says on every launch, keeping the part
  specific to a slice: "A new type means a new real-world concept" (Build
  around the real thing); "Stay in scope… `scratch/questions.md`" (Finish the
  plan; don't grow it); the generic half of "Write the tests"; the closing
  paragraph on staying in this checkout and starting no second Task (Tasks
  section). "Finish the slice by deleting" keeps its test-deletion rule.
- Runs unchanged as a non-decider in `ui`, `optimize` and this repository's
  `.lf/graphs/*`; `.lf/skills/implement.md` extends it and needs no edit.

**`task/skill/verify.md`** — identical-tree fast path, placed before "Check
headless first":
- When the tree qa and gate checked is the tree that merged (`git diff
  <final PR head> <merge SHA>` prints nothing), post-merge CI for that merge
  SHA is green, and no claim depends on something that only exists after
  merge (a release install, a deployment, production state), the claims were
  already checked on these exact files: verify from the qa and gate records
  and the CI run, cite them, and don't rebuild main.
- Any of the three missing means the full check, as today.
- Brief says `git diff <qa-head> origin/main`. The plan compares against the
  merge SHA: `origin/main` keeps moving, and a squash onto a newer base then
  shows other people's changes, which correctly sends it down the full path.
  When nothing has landed since, the two are the same commit.

**Generic wording**
- `kickoff.md:26`: "Linear's API has no conditional assignment…" becomes "the
  tracker's API has no conditional assignment…".
- `advance.md:22`: "No Task, Wave or Linear setup" becomes "No Task, Wave or
  tracker setup". Its `description:` ("the current Loopflow work") stays: it
  names the tool the skill drives, as `prompt.md` does.
- `write-docs.md` step 4: `lf open --file <path>` (repeat `--file`), which
  shows the files in an attached Files view and never launches anything; with
  none attached, report the paths. Checked in
  `rust/loopflow/src/lf/commands/open/files.rs`: host `auto` is accepted and
  prints "No Desktop Files view attached; nothing opened." instead of failing.
  Frontmatter `produces:` ends "with their paths shared", no "Desktop".
- `ship-review.md` gather list: the deferred findings now come from the plan's
  heading that `realign` writes, with `scratch/concept-review.html` as the page
  behind them. One line changes.

## Everything else that names the old order

Tests (expected values change; no production Rust changes):
- `rust/loopflow-execution/src/graph/compiled.rs:603` and `:685`
  (`pursue_loops_to_a_published_pr`): labels become kickoff, concept-review,
  realign, write-docs, sync, implement, concept-review, realign, compress, qa,
  pr-publish; returns are step 5 → step 4 and step 7 → step 4.
  `steps[4].sources == ["pursue", "build"]` still holds (step 4 is still sync).
- `rust/loopflow-execution/src/graph/mod.rs:1542`: edges become
  `[("implement", 4), ("realign", 4)]`.
- `rust/loopflow/src/lf/commands/graph.rs:1646`: add `fix` to the list with no
  interactive step and `ui` to the list with one.
- `rust/loopflow-execution/src/task_graph.rs:759` (concept-review placed in
  `plan` then `build`) holds unchanged.
- `tests/goldens/builtin_implement.md`, `builtin_realign.md`: regenerate with
  `uv run python tests/goldens/update_goldens.py`.
- DTO fixtures under `tests/fixtures/dto/` and the Swift tests name no step
  order from these Graphs (grep for `concept-review|write-docs` finds none).

Docs:
- `docs/orchestration.md`: the `build` listing and its prose (126–155: who
  decides each loop, `realign` after review), the `feature` listing's
  comments (220–223), and a short entry each for `ui` and `fix`. `fix` says
  plainly that it has no step for a person and merges when checks pass.
- `docs/index.md:122`: `implement → sync → realign`.
- `docs/surfaces.md:285`: the Start menu sample gains `ui` (1 stop) and `fix`
  (never stops).
- `docs/work.md:549` and `swift/README.md:407`: read and keep if still true.

## Choices made for review

1. **`implement` advances, not iterates, on a broken plan.** `realign` is
   where replanning happens in `build`, and a fresh review sits between them.
   Passed over: letting `implement` rewrite the pass list and iterate, which
   is how the 9–17 thin passes of LOO-1082 arose.
2. **`realign` records each finding's fate in the plan.** Without it the
   `realign` inside `queue`'s `refresh` rereads a stale page and reopens
   settled findings. Passed over: stamping the page with a commit and having
   `realign` compare; that puts a protocol in two skills.
3. **`verify` compares against the merge SHA** (above).
4. **`ui` keeps `pr-publish` before `deliver`** as the brief has it, though
   `gate` inside `deliver` publishes too. It costs one cheap step and leaves a
   PR up when `deliver` is skipped with `--from`.
5. **`demo.md` is not edited.** Headless, it says the demo didn't happen and
   finishes, which as `ui`'s decider is `advance`: a `ui` Task with auto on
   lands without anyone having used it. That is what auto means everywhere
   else; noted in `scratch/questions.md`.

## Known costs of the new order

- `concept-review` sees the change before `compress` and `qa`, so it never
  reads qa's findings. A product-level problem qa finds goes to
  `scratch/qa.html`, which `ship-review` already gathers; in `pursue`, which
  has no `ship-review`, it reaches the person only through the PR.
- `sync` now runs before each `implement`, so the last pass is not synced
  before `compress` and `qa`. `gate` and `land --wait-and-fix` sync later.
- `fix` has no interactive step, so `lf task run` on it may merge with nobody
  watching. Its description and docs say so.

## Left out

- `loop-or-next`, `demo`, `qa`, `compress`, `gate`, `reflect` text.
- This repository's `.lf/graphs/*` (they use `loop-or-next` and their own
  order) and `.lf/skills/*`.
- Choosing `ui` or `fix` automatically from a Task's content; a person or a
  Wave default picks the Graph.
- `.lf/wave/intelligence/MEMORY.md` still says `realign` owns memory curation
  (2026-09-28). It is read-only here; the correction is proposed in
  `scratch/questions.md`.

## What goes

- `concept-review.md`: the headless plan-writing mode and its outcome report.
- `realign.md`: `## Wave memory` and the Wave-memory limit paragraph (moved,
  not copied).
- `implement.md`: the `@dataclass` example and the rules `LOOPFLOW.md` repeats.
- The old expected step lists in the three Rust tests above.

Must survive: `concept-review` with a person (the page stands in for code
review; nothing else changes unless asked); `realign` in `refresh`; `implement`
as an ordinary step; `verify`'s three outcomes.

## Acceptance check at gate

```bash
cargo build -p loopflow
target/debug/lf graph show build    # the seven lines under Demo
target/debug/lf graph show code     # the sixteen steps listed under Demo
target/debug/lf list graph | grep -E '^  (ui|fix) '
grep -rnE "Linear|Loopflow Desktop|--host desktop" \
  rust/loopflow-definitions/src/builtins/task/skill/{kickoff,advance,write-docs}.md   # no output
uv run python scripts/test.py --reuse-passing \
  --rust-filter 'test(graph::) | binary(golden_prompt) | test(documented_commands::) | test(builtins::)'
```

Run `lf` from an empty temp repository with `LF_*` unset, so this checkout's
`.lf/graphs/` and `.lf/skills/` do not stand in for the builtins.

## Passes

One pass; it is prose, four YAML files and test expectations.

1. The four Graph files, then the three Rust tests. This is the part that
   could prove the plan wrong, and the probe above already ran it.
2. The skills: `concept-review`, `realign` with `wave_operate`, `implement`,
   `verify`, the three wording fixes, the `ship-review` line. Regenerate
   goldens.
3. Docs.
4. Read each changed skill once in every Graph position it holds (listed per
   skill above) before calling it done.

Done when `lf list graph` shows `ui` and `fix` beside the reordered `build`
and `plan`; `lf graph show code` runs review and realign before compress and
qa; a headless `concept-review` leaves `scratch/plan.md` untouched; and the
gate command above passes.
