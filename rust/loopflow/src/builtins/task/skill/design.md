---
requires: none
produces: scratch/<branch>.md
action_style: exploratory
---
Dream big, detail the idea, then shape it into one exact design.

Start from the problem and the experience wanted; a Task is optional. Explore
solutions before choosing one, and keep observations, proposed mechanisms, real
constraints and what people said apart. The design holds the architecture,
sequencing and acceptance checks. Keep the Task's original problem when the
solution changes.

Read `scratch/` and `AGENTS.md`. Continue an existing design rather than
restarting; a newly filed Task doesn't restart discovery, and launching a draft
doesn't accept it. Keep what people said, open questions and the selected
Graph. Read a Wave's `GOAL.md` and `MEMORY.md` only when placement is part of
the design and the seed names that exact Wave; run `lf wave status <wave>` only
when its chapter state matters. Never guess a Wave.

Write the design to `scratch/<workspace-slug>.md` and open questions to
`scratch/questions.md`. Write down what a later agent couldn't work out again
from the code, git or a fresh run: what people said, word for word, with the
speaker's name, what a run showed, outside facts. "Looks fine" stays "looks
fine"; never turn a reply into "accepted" or "chose". Cut the rest.

## Surface

**Interactive:** work in the conversation. Find out what they want to build and show the exact final design. Ask specific numbered questions so "2b" is a complete answer, not "does this look good?".

**Headless:** infer intent from the Task directive, quoted words and the parent's findings. Write the best complete draft without claiming User confirmation. Record real ambiguity in `scratch/questions.md`.

## Workflow

### 1. Dream

Let the whole idea come out before scope pressure. Follow what's surprising.
Quote the words that anchor intent, constraints or priority verbatim in the
design.

### 2. Detail

Make it concrete: data structures, public functions, interactions, edge cases,
failure recovery, and the real demo. Write as you go so a crashed Session loses
no decisions.

Name the shape:

- **Additive series:** independently valuable increments. Keep later ones at
  intent level; each gets its own design when launched.
- **Indivisible change:** detail the full architecture and say it proceeds in
  internal slices but ships as one PR. Keep the target architecture,
  deletion path, forbidden near-misses and acceptance conditions fixed while
  `This slice` moves.

A redesign deletes what it replaces. Before choosing an approach, find earlier
attempts: partial implementations, unreachable branches, hidden commands,
unused tables. List them, with their exclusive tests and fixtures, under
**Delete — do not maintain** so implementation removes them in the same diff
and never repairs them. Name the behavior, data and tests that must survive.
Sequence the deepest cuts first; if removal needs a consumer cutover or
migration, put the minimum of it in the same cut instead of first modernizing
the old path. Additive work needs no deletion.

Plan one schema migration per Task, written against the last released schema
and edited in place until it lands; never schedule intermediate schemas.

### 3. Size-check

A design past roughly 1,000 words or an implementation past roughly 1,000 lines
is a signal, not an automatic split. Split additive work into a keystone plus
follow-ups; keep an indivisible change whole. Don't create follow-up Tasks here.

### 4. Place

For existing or requested managed work, pick exactly one Wave whose objective
fits; the current chapter follows. If none fits, say placement is open. A
standalone design needs no Wave, Task or placement. If a lookup fails, note the
command, date and error; keep Machine state out of the design.

Tighten the design to:

- **What to build** — one sentence on the new end state.
- **Placement** — managed work only: the Wave, or that it's open.
- **The demo** — the real action and what appears.
- **Data structures** — core domain values.
- **Key functions** — signatures and intent.
- **Constraints** — choices that force a rewrite if guessed wrong.
- **Done when** — headless gate command and expected result.
- **Current system** — concepts, writers and paths the change reshapes or
  deletes.
- **Delete — do not maintain** — removal targets, their exclusive tests and
  fixtures, and cutover dependencies; keep it current.
- **Forbidden outcomes** — duplicate representations, compatibility layers, or
  locally passing states that still break the intended architecture.
- **Internal slices** — deepest deletions first, one coherent cut marked
  `This slice`, remaining work kept current.
- **Measure** — only when a real before/after number exists.

For an additive series, describe the keystone fully and list the follow-ups
precisely enough for `launch-plan` to file. Before finishing, reread the design
and present the scope, the keystone's edge, follow-ups and open assumptions.

When filing or editing a Task, keep its description to the current problem,
desired outcome, acceptance and real constraints. Dated updates go in Task
comments with links. Comments may be collapsed, so blockers, dependencies and
accepted scope stay in the description. Rewrite the description when scope
changes; don't append amendments.

## Checks run unattended

Specify headless acceptance commands and expected results for gate to run once.
Desktop uses app builds and view/interaction tests. Never require a person,
display session or permission dialog; leave unavailable checks to CI. Judgment
belongs to demo and review.

## Handoff

Leave the design ready for kickoff to turn into an implementation plan. Keep
what will be built, remaining work and one check-result line;
no session instructions or Machine facts, and past skill names without `$`.
Allocating and launching Tasks is launch-plan's job when execution is asked
for; finishing a design doesn't authorize launching work.
