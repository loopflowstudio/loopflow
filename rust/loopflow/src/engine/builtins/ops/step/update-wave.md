---
requires: diff vs main | scratch/ analysis | both
produces: wave/<wave>/ (GOAL.md, MEMORY.md), roadmap updates in Linear, scratch/ cleanup
---
Single owner of `wave/<wave>/`. Keeps the wave's identity current and folds what the branch learned into memory.

## The wave's shape

A wave is two local files plus a remote roadmap:

- **`wave/<wave>/GOAL.md`** — the wave's identity: what it's for, how it judges
  progress, the loop prompt it runs. Frontmatter carries machine config and the
  Linear handle (`pm.linear_project`). This is the anchor; it changes rarely.
- **`wave/<wave>/MEMORY.md`** — what the wave remembers between loops. Durable
  observations, decisions, and context. This is where branch learnings land.
- **The roadmap lives in Linear**, not in the repo. Read it with `lf op pm show`;
  change it with `lf op pm update`. There is no local roadmap mirror — never
  write `N-*.md` item files or a roadmap table.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` completely — design docs and notes for the current work live
  here (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds
  open questions and assumptions).
- Read `wave/<wave>/GOAL.md` and `MEMORY.md`.
- Read the live roadmap: `lf op pm show` (add `--wave <name>` if ambiguous).
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

## Goal

Whether you're cleaning up after a build, reconciling scratch analysis, or both:

- Durable learnings from `scratch/` are folded into `MEMORY.md`.
- `GOAL.md` still describes the wave truthfully — if the branch changed the
  wave's intent, flow, or metrics, update it. Otherwise leave it.
- The roadmap in Linear reflects reality: shipped work is closed, new work is
  filed, stale items are corrected — all through `lf op pm update`.
- `scratch/` is trimmed to what a reviewer needs (see below).

## Bias: fold into MEMORY, don't drop

`scratch/` is cleared on land. Anything left there is lost. Anything folded into
`MEMORY.md` — or pushed to the roadmap — survives. **Dropping content is a worse
failure mode than duplicating it.**

Every scratch file with future-relevant content must land somewhere durable:

- Decisions, learnings, gotchas, patterns established → fold into `MEMORY.md`.
- Concrete future work (a next step, a follow-up, a discovered bug) → file it on
  the roadmap with `lf op pm update --title "…" --notes "…"`.
- Open questions about future work → `MEMORY.md`, or a roadmap item if it's
  actionable.
- If content overlaps what's already in `MEMORY.md`, merge it — don't skip it.

Design docs for already-shipped work and other purely historical content can be
left for git history. The test: does this content inform future work? If yes,
fold it. If it only describes what was already built, let it go.

## Workflow

1. Read the diff (if any) to understand what this branch built.
2. Read `GOAL.md`, `MEMORY.md`, and the live roadmap (`lf op pm show`).
3. Read `scratch/` — every file, completely.
4. **Reconcile the roadmap against reality.** For each item on the Linear
   roadmap, ask:
   - **Shipped?** If this branch (or main) crossed its finish line, close it:
     `lf op pm update --id <task-id> --status done`.
   - **Accurate?** If the item's description no longer matches the codebase,
     correct it: `lf op pm update --id <task-id> --title "…" --notes "…"`.
   - **New work surfaced?** File it: `lf op pm update --title "…" --notes "…"`.
   Do this remotely. Never create or delete local roadmap files.
5. **Fold scratch learnings into `MEMORY.md`.** Merge into existing sections
   where there's a clear match; add sections for new durable context. Keep it
   tight — memory is a working store, not an archive.
6. **Update `GOAL.md` only if the wave's identity moved.** Changed objective,
   changed measures, or changed routing judgment count. If the branch didn't
   change what the wave *is*, leave `GOAL.md` alone.
7. **Trim scratch docs for shipped work.** Don't delete them — `lf op pr land`
   handles that. Strip implementation detail that now lives in the code. Keep
   only:
   - **Validation procedures** — "Done when" checks, commands to run, expected
     output.
   - **Measurement instructions** — benchmarks, before/after, how to reproduce.
   - **Try-it recipes** — quick ways for a reviewer to exercise the change.
   If a scratch doc has none of these, delete it.

## Creating a new wave

When `scratch/` holds a proposal and no wave exists yet, create one:

1. Write `wave/<wave>/GOAL.md` (see below).
2. Create `wave/<wave>/MEMORY.md` — seed it with the load-bearing context from
   the proposal (key decisions, constraints, what's known). It can be short.
3. Connect the roadmap: `lf op pm init --wave <name>` creates/links the wave's
   Linear project and writes `linear_project` into `GOAL.md`.
4. File the opening roadmap items in Linear with `lf op pm update` — the urgent
   and next-step work, one task each. The roadmap starts in Linear, not on disk.

### GOAL.md

`GOAL.md` anchors the wave's identity. Loopflow parses it for the UI.

**Frontmatter:**

```yaml
---
pm:
  linear_project: "8c4ba3f9-cf23-4136-87ed-37847aa7dc82"   # written by `lf op pm init`
---
```

**Body** — the loop prompt, in the wave's own voice:

- What this wave is and why it exists; scope boundaries as natural qualifiers.
- How it judges progress — the metrics that matter (numeric where possible).
- The milestones or shape of the work ahead.

**GOAL.md must not contain:** a roadmap table, status indicators
(shipped/in-progress/planned), or item lists. The roadmap is in Linear.

## Coherence

Roadmap items go stale — the codebase moves, other waves ship, intent evolves.
When you find incoherent items on the roadmap, fix them in place through
`lf op pm update`:

- **Finish line moved** — the goal was reached a different way → close the item.
- **Design diverged** — building it as written would fight the current
  architecture → rewrite its title/notes to match reality.
- **Value diminished** — the 80% case is solved, the remainder is marginal →
  close it or rewrite it down.
- **Items overlap** — two items describe the same work → close one, sharpen the
  other.

This is housekeeping, not new work — the wave maintaining its own coherence. It
doesn't need human review.

## Silence

A wave with a current `GOAL.md` and `MEMORY.md` but nothing new to build is
**silent** — alive, watching its area, not proposing work. That's a healthy
state, not a failure. An empty roadmap is fine. Shipping mediocre work to avoid
being empty trains the user to ignore the wave; staying quiet until there's
something genuinely compelling earns trust that compounds.

Keep the wave (its `GOAL.md`/`MEMORY.md` are its identity and sensor). Delete a
wave's directory only when it's standalone and its purpose is truly done, or the
human explicitly closes it.

## Output

Updated `wave/<wave>/MEMORY.md` (and `GOAL.md` if intent moved), a roadmap in
Linear that reflects reality, and a trimmed `scratch/`.

**"No changes needed" is only valid when scratch/ is empty and the roadmap
already matches reality.** If scratch has files, something must move into
`MEMORY.md`, the roadmap, or both.
