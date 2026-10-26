---
requires: a Task brief or kickoff plan
produces: a chosen direction recorded in the design, with the visions it was chosen from
default_agent: claude
action_style: exploratory
---
Show several distinct visions of the outcome so the person can say "this one, build it."

## Read what exists

Read the Task, the current plan and the risks it found. Its findings constrain
the visions; its chosen approach is only one candidate. Read enough of the
system to make each vision concrete.

## Dream three visions

Each vision is a coherent world, not a variant of presentation. Let them differ
by mechanism or by the experience they promise. At least one is radical
success: what this would be if it worked far beyond the brief. Give each a
short title and one line on what is true in that world. Keep each one's cost
and risk visible next to its appeal.

Take this as inspiration, not regulation. A Task that really has one obvious
answer says so and spends the time on its open question instead. Two visions
or five can serve better than three.

## Make them tangible

For work with a human-facing experience, build one page,
`scratch/explore.html`: one panel per vision, real-looking content, and the key
interaction working. For other work, pick the form that makes the difference
visible: CLI transcripts, data-model sketches, a day in the life of the system,
the failure each one avoids. Put that on the page too.

The page is what the person reviews; the conversation is about what they see
there. Make it a good page:

- One self-contained `.html` under `scratch/`: inline CSS and script, no
  network, no build step. Escape code. Readable in light and dark and at narrow
  widths.
- Lead with the decision you need. End with numbered questions, so "2b" is a
  complete answer.
- Show the thing, not a description of it. Mark proposed against existing
  behavior visibly.
- One reading column, generous whitespace, no dashboard chrome or badges. Shape
  the structure to the content; no fixed template.
- Header: Task, step, date, and the files it is built from.
- Say its path. Loopflow Desktop shows `scratch/` pages beside the
  conversation; in a bare terminal, open it in the browser. Regenerate the same
  path each round, and write it last, after updating the design.

## Follow their lead

The first page is an opening move, not a ballot. The person may pick one
outright, ask for A + B, or say "three more, keep X fixed and vary Y." That can
repeat for many rounds. Answer each request as given and regenerate the page.
Do not steer toward closure, summarize a choice they have not made, or turn a
remark into a decision.

## Record what they settle

Write into the design only what the person has decided, with their name and
the date, and keep open threads open. The page is a view; the decision lives in
the Markdown design. Once they say "build this," the design reflects that
direction well enough to implement: update its slices and acceptance checks
where the direction changed them, and link the latest page as evidence.

## Headless

Produce the visions and the page, recommend one, and leave the choice open in
`scratch/questions.md`. Never record acceptance.
