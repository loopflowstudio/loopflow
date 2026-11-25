# LOO-1104: review before compress and qa; `ui` and `fix` Graphs; tighter builtin skills

## Outcome

Status: built (`78bde1d766`), compressed (`c6eab6e222`), QA'd (`73abaa380c`), and reviewed (`scratch/concept-review.html`). Two small prose seams remain for `ship-review` / `gate` (see `## Post-build review findings`).


A Task on `code`, `feature`, `pursue` or `redesign` runs `compress` and `qa`
once, after the design has settled, instead of once per outer-loop pass. The
measurement behind it (14 days, 5,796 sessions, 154 Tasks): 70.3% of qa spend
($1,063 of $1,512) and 73.7% of compress spend ($387 of $525) were followed by
later implement passes; $735+ (82 qa + 76 compress passes) was invalidated by
the very next concept-review iterating `build`.

Two new Graphs appear wherever Graphs are listed (`lf list graph`, Desktop's
Start menu and New Task sheet): `ui`, which puts the running UI in front of a
person every pass (`1 stop`), and `fix`, which takes a bounded bug from brief
to merged with no planning or review steps (`merges without asking`).

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
  fix     loop(cmd: sync → implement) → qa → gate → cmd: land --wait-and-fix
  plan    kickoff → concept-review → realign → write-docs
  ui      kickoff → loop(cmd: sync → implement → qa → demo (interactive)) → compress → deliver
```

`code`, `pursue`, `feature` and `redesign` compose over `plan` and `build`
with no edit of their own (`code` compiles to kickoff, concept-review,
realign, write-docs, sync, implement, concept-review, realign, compress, qa,
pr-publish, ship-review, reflect, gate, land, verify).

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
    - cmd: sync
    - implement
    - qa
    - interactive: demo     # you use it; iterate goes back to sync → implement with your words
- compress
- graph: deliver
```

```yaml
# fix.yaml
description: Fix a bounded bug or small tweak and land it.
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
- The decision prompt reads `"Decision pass N. Iterating repeats K steps, back to <first step>"`
  (`rust/loopflow/src/lf/commands/graph.rs:1263`). In `build` the first step is
  now `sync`. `GraphOutput::Decision` (`advance` / `iterate` / `blocked`) is
  attached only when `skill.returns.is_some()` (`graph/output.rs:36`).
- Desktop's Start menu (`swift/LoopflowMac/Views/BacklogView.swift:128-145`)
  inspects ops before the first stop and already labels `fix` as
  **`merges without asking`** under `Run without stopping`, and `ui` as
  **`1 stop`** under stopping workflows. `lf task auto off` still stops `fix`
  before `cmd: land --wait-and-fix` (`stopped_landing`, `graph.rs:907`).

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
  review against the base as a first-time reader; show a product choice on the
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
    with the question on a product choice that can't be undone once shipped.
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
  - With no `scratch/plan.md` (the brief is the plan, e.g. `fix`): don't
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
- Runs unchanged as a non-decider in `ui`, `optimize` and `.lf/graphs/*`;
  `.lf/skills/implement.md` extends it and needs no edit.

**`task/skill/demo.md`** — align `## Headless` with `ui.yaml` (where `demo` is
now a loop decider under `auto on` / `lf -b`):
- Keep "With nobody there, nobody can feel it... Don't act out a person's
  reactions or invent feedback."
- Add two lines for when standing in as a loop decider: inspect what `qa`
  produced (`scratch/qa.html`, screenshots, transcripts); return `iterate` only
  when promised UI behavior or visual quality is visibly broken, otherwise note
  that no person tried it and `advance`.

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
- `rust/loopflow/src/lf/commands/graph.rs:1646`: add `fix` to the list with no
  interactive step and `ui` to the list with one.
- `rust/loopflow-execution/src/task_graph.rs:759` (concept-review placed in
  `plan` then `build`) holds unchanged.
- `tests/goldens/builtin_implement.md`, `builtin_realign.md` (and any golden
  covering `wave_session` or `demo` if present): regenerate with
  `uv run python tests/goldens/update_goldens.py`.
- DTO fixtures under `tests/fixtures/dto/` and Swift tests name no step order
  from these Graphs.

Docs (written in `write-docs`; read before implementation and keep in sync with `ui.yaml` description):
- `docs/orchestration.md`: updated the `build` listing and prose (`implement` decides inner loop, `realign` decides outer loop, `Decision pass` example says `back to sync`), updated `feature` listing comments, and added `ui` and `fix` to the Built-in Graphs table with examples and notes.
- `docs/index.md` and `README.md`: replaced stale `auto` in the Built-in Graphs list with `ui`, `fix`; updated `docs/index.md` loop example to `sync → implement` inside `concept-review → realign`.
- `docs/surfaces.md`: updated Start menu sample with `ui` (`1 stop`) under stopping workflows and `fix` (`merges without asking`) under `Run without stopping`, with a note on `merges without asking` and `lf task auto off`.
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
2. **`ui.yaml` drops `pr-publish` before `deliver` and puts `cmd: sync` before
   `implement`.** `deliver`'s `gate` already publishes after `reflect` (avoiding
   an extra LLM step and a cancelled CI run), and syncing before `implement`
   matches `build`, `fix`, and `deep-clean` while avoiding mid-loop conflicts
   after a long human demo pause. Passed over: keeping `pr-publish` and
   `implement → cmd: sync` as drafted.
3. **`demo.md` headless mode inspects `scratch/qa.html` when standing in as a
   loop decider.** Makes `lf task auto on` / Auto-Ship on `ui` deterministic
   and consistent with `ship-review` and `stand_in_message`. Passed over:
   leaving `demo.md` unedited (which clashes with `stand_in_message`) or
   blocking Auto-Ship on `ui`.
4. **`implement` broken-plan rule keys off `scratch/plan.md` presence.**
   `implement` cannot see what steps follow its loop in the Graph. With
   `scratch/plan.md` (`build`), it notes what broke and advances to
   review/realign; without `scratch/plan.md` (`fix`), it returns `blocked` so
   broken work never proceeds to `qa → gate → land`. Passed over: letting
   `implement` rewrite the pass list and iterate, or asking it to guess Graph
   topology.
5. **`verify` compares `git diff <qa head> <merge SHA> -- . ':!scratch'`.**
   Comparing `<qa head>` (not `<final PR head>`) catches post-`qa` edits by
   `gate`/`sync`/`land`; comparing `<merge SHA>` (not moving `origin/main`)
   avoids false diffs from later PRs; excluding `scratch/` keeps `lf scratch
   clear` on `land` from breaking the fast path on every PR.
6. **`fix` merges with no interactive step.** Matches the brief; Desktop's
   Start menu already warns `merges without asking`, and `lf task auto off`
   still stops before `land`. Passed over: adding `interactive: verify` at the
   end.

## Known costs of the new order

- `concept-review` runs before `compress` and `qa`, so it never reads qa's
  findings. Product-level problems qa finds go to `scratch/qa.html`, which
  `ship-review` gathers; in `pursue` (no `ship-review`) they reach a person
  through the PR.
- `sync` runs before each `implement` (in `build`, `ui`, `fix`), so the final
  pass is not synced before `compress` and `qa`. `gate` and
  `land --wait-and-fix` sync later.
- In `ui`, `compress` runs after `interactive: demo`, so a refactor in
  `compress` is checked by `gate` + CI (including headless view tests) rather
  than re-demoed by a person.
- `fix` has no interactive step, so `lf task run` merges as soon as checks
  pass unless `lf task auto off` is set.

## Left out

- `loop-or-next`, `qa`, `compress`, `gate`, `reflect`, `wave_operate` text.
- This repository's `.lf/graphs/*` and `.lf/skills/*`.
- Choosing `ui` or `fix` automatically from a Task's content; a person or Wave
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
as an ordinary step; `demo` interactive; `verify`'s three outcomes.

## Acceptance check at gate

```bash
cargo build -p loopflow
# Run from an empty temp repo with LF_* unset so repo .lf/ doesn't shadow builtins:
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') sh -c '
  tmp=$(mktemp -d) && git -C "$tmp" init -q &&
  (cd "$tmp" && "$PWD/target/debug/lf" graph show build &&
   "$PWD/target/debug/lf" graph show code &&
   "$PWD/target/debug/lf" list graph | grep -E "^  (ui|fix) ")
'
grep -rnE "Linear|Loopflow Desktop|--host desktop" \
  rust/loopflow-definitions/src/builtins/task/skill/{kickoff,advance,write-docs}.md   # no output
uv run python scripts/test.py --reuse-passing \
  --rust-filter 'test(graph::) | binary(golden_prompt) | test(documented_commands::) | test(builtins::)'
```

## Passes

One pass; it is prose, four YAML files, docs, and test expectations.

- Built in one pass (`78bde1d766`), compressed (`c6eab6e222`), and QA'd (`73abaa380c`):
  - Reordered `build.yaml` and `plan.yaml`; added `ui.yaml` (two-sentence description so Desktop's Start menu fits on one line, synced with `docs/surfaces.md`) and `fix.yaml`.
  - Updated `concept-review.md`, `realign.md`, `wave_session.md`, `implement.md` (109 lines after QA tightening), `demo.md`, `verify.md`, `kickoff.md`, `advance.md`, `write-docs.md`, `ship-review.md`; regenerated `tests/goldens/builtin_{implement,realign}.md`.
  - Updated Rust graph expectations in `compiled.rs` (including `fix` and `ui` cases and pinned return edges), `graph/mod.rs` (`edges()` reports relative offsets `1` and `3`, not absolute indices), `commands/graph.rs`, and `task_graph.rs:631` (`write-docs` shifted by +1 after `realign` joined `plan`).
  - QA (`scratch/qa.html`) verified `lf graph show`, `lf list graph`, Desktop `StartFlow` labels, `lf help`, and real Claude haiku decider runs for `build`'s inner loop, `fix` bounded brief, read-only `concept-review`, and `fix` oversized brief (caught and fixed in `73abaa380c`: no-plan `implement` now blocks oversized briefs before writing any plan or code).
- Remaining: two small prose seams for `ship-review` / `gate` below; the Graph model, ownership map, code, docs, and tests hold.

## Post-build review findings

Recorded by post-build `concept-review` against base `23ce1a69f9` (`scratch/concept-review.html`). The overall model holds; neither item warrants re-running the 6-step `build` loop, and both fit cleanly in `ship-review` / `gate`:

1. **`realign.md` reversible-choice handoff (`rust/loopflow-definitions/src/builtins/task/skill/realign.md`, plus `tests/goldens/builtin_realign.md`):** `concept-review.md:48-50` shows a reversible product choice as a recommendation with what was passed over, and `ship-review.md:13` reads "choices agents made" from `scratch/plan.md`, while `realign.md:25-26` says "A change that needs a product decision is written as the exact open choice. Nothing goes into the plan as settled unless someone said so" and `## After a review` omits reversible product choices. Add 2–3 sentences to `realign.md` so headless `realign` adopts a recommended reversible choice into the plan marked "made for review" (with what was passed over, never written as settled), while irreversible choices still `blocked` (in a loop) or go to `scratch/questions.md` (outside a loop), then regenerate `tests/goldens/builtin_realign.md`.
2. **`verify.md` `<qa head>` pointer (`rust/loopflow-definitions/src/builtins/task/skill/verify.md:18`):** Clarify that `<qa head>` is the branch's `lf qa:` commit (from `scratch/qa.html` finding #3).

## Deferred findings

- **Update `.lf/wave/intelligence/MEMORY.md`:** read-only in a Task checkout; replace the 2026-09-28 entry ("`realign` now owns memory curation") with "`wave-session` owns memory curation" in an intelligence Wave session once LOO-1104 lands.
- **Step-commit `-a` propagation (`scratch/qa.html` #1):** with `-a claude:haiku` and no `default_agent`, `lf commit --step` falls back to `lf <skill>: checkpoint`. Outside LOO-1104's scope; `.lf/qa/skills.md` documents the current behavior.
- **Repo-local `.lf/graphs/pursue-auto.yaml` & automatic Graph selection:** separate outcomes outside this Task.

Done when `lf list graph` shows `ui` and `fix` beside the reordered `build`
and `plan`; `lf graph show code` runs `concept-review` and `realign` before
`compress` and `qa`; a headless `concept-review` leaves `scratch/plan.md`
untouched; `wave-session` is the single owner of Wave memory curation; `verify`
has the `scratch`-excluded identical-tree fast path; and the gate check passes.
