---
requires: a Task brief
produces: a chosen direction recorded in the design, with the visions it was chosen from
action_style: exploratory
---
Show several distinct visions of the outcome so the choice can be "this one, build it."

## Read what exists

Read the Task, `scratch/` and what people have already said. An existing design
or a suggested approach is only one candidate. Read enough of the system to make
each vision concrete and to know what each would cost.

## Dream three visions

Each vision is a coherent world, not a presentation variant: they differ by
mechanism or by the experience they promise. At least one is radical success,
what this would be if it worked far beyond the brief. Give each a short title,
one line on what's true in that world, and its cost and risk next to its
appeal.

Inspiration, not regulation. A Task with one obvious answer says so and spends
the time on its open question. Two visions or five can beat three.

## Make them tangible

For anything people see or touch, build one page, `scratch/explore.html`: one
panel per vision, real-looking content, the key interaction working. For other
work, pick the form that shows the difference: CLI transcripts, data-model
sketches, a day in the life of the system, the failure each one avoids. Put
that on the page too.

The page is what gets reviewed, so make it good:

- One self-contained `.html` under `scratch/`: inline CSS and script, no
  network, no build step. Escape code. Readable in light and dark and at narrow
  widths.
- Lead with the decision needed. End with numbered questions, so "2b" is a
  complete answer.
- Show the thing, not a description of it. Mark proposed against existing
  behavior.
- One reading column, generous whitespace, no dashboard chrome or badges. Shape
  the structure to the content.
- Header: Task, step, date, and the files it's built from.
- Say its path. Loopflow Desktop shows `scratch/` pages beside the
  conversation; in a bare terminal, open it in the browser. Regenerate the same
  path each round, and write it last, after updating the design.

## Follow their lead

The first page is an opening move, not a ballot. The answer may be one outright,
A + B, or "three more, keep X fixed and vary Y," for many rounds. Answer each
request as given and regenerate the page. Don't steer toward closure, summarize
a choice not yet made, or turn a remark into a decision.

## Record what was said

Write into the design what will be built, and beside it the exact words people
used, with the speaker's name. "Looks fine" stays "looks fine"; never turn a
reply into "accepted" or "chose". To settle an open thread, ask a specific
numbered question so "2b" is a complete answer. Keep open threads
open; skip what the page or code already shows. The page is a view; the
decision lives in the Markdown design. After "build this," write the chosen
direction into the design for kickoff to plan, and link the latest page.

## Headless

Produce the visions and the page, recommend one, and leave the choice open in
`scratch/questions.md`. Never record acceptance.
