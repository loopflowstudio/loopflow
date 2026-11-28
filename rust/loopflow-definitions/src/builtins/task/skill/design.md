---
requires: none
produces: scratch/plan.md
action_style: exploratory
---
Dream big, detail the idea, then shape it into one exact design.

Start from the problem and the experience wanted; a Task is optional. Explore
solutions before choosing one, and keep four things apart: what you observed,
mechanisms you propose, real constraints, and what people said. Keep the
original problem when the solution changes.

Read `scratch/` and the agent guide, and continue an existing design rather
than restarting. Write the design to `scratch/plan.md` as you go,
so a crashed Session loses no decisions, and open questions to
`scratch/questions.md`. Quote the words that anchor intent, with the speaker's
name. "Looks fine" stays "looks fine".

## In conversation

Work with the person directly: find out what they want to build and show the
exact design. Ask specific numbered questions so "2b" is a complete answer,
never "does this look good?".

## Headless

Write the best complete draft in `scratch/plan.md` from the brief and the words
quoted in it, record real ambiguity in `scratch/questions.md` and label your
recommended path so a later choice takes precedence. In your final answer, link
`scratch/plan.md`, state the recommended next action, and pose numbered choices
so someone can answer from the conversation; a draft is not a decision.

## 1. Dream

Let the whole idea come out before scope pressure. Follow what's surprising.

## 2. Detail

Make it concrete: data structures, public functions, interactions, edge cases,
failure and recovery, and the real demo.

Name the shape. An **additive series** is independently valuable increments:
detail the first and keep the rest at intent level. An **indivisible change**
gets its full architecture, built in internal slices and shipped as one PR.

A redesign deletes what it replaces. Before choosing an approach, find the
earlier attempts: partial implementations, unreachable branches, hidden
commands, unused tables. List them, with their tests and fixtures, under
**Delete — do not maintain**, so implementation removes them in the same diff
and never repairs them. Name the behavior and data that must survive. Deepest
cuts go first.

## 3. Size-check

A design past about 1,000 words, or an implementation past about 1,000 lines,
is a signal, not an automatic split. Split additive work into a keystone plus
follow-ups; keep an indivisible change whole.

## 4. Tighten

- **What to build** — one sentence on the new end state.
- **The demo** — the real action and what appears.
- **Data structures** — core domain values.
- **Key functions** — signatures and intent.
- **Constraints** — choices that force a rewrite if guessed wrong.
- **Done when** — a headless command and its expected result.
- **Delete — do not maintain** — removal targets and what must survive.
- **Slices** — a suggested order to build in, each running end to end; the
  first is the one that could prove the plan wrong.
- **Measure** — only when a real before and after number exists.

For managed work, name the one Wave whose objective fits, or say placement is
open; never guess a Wave. A standalone design needs none.

Reread the design and present the scope, the keystone's edge, the follow-ups
and the open assumptions. Finishing a design launches nothing and files
nothing.
