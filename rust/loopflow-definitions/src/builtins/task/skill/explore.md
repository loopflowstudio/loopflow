---
requires: a Task brief
produces: a page comparing the visions and a direction for planning
action_style: exploratory
---
Show several distinct visions of the outcome so the choice can be "this one, build it."

Read the Task, `scratch/` and what people have already said. Read enough of
the system to make each vision concrete and to know what it costs.

## Dream the visions

**Differ by mechanism, not presentation.** Each vision is a coherent world: a
different mechanism, or a different experience promised. An existing design or
a suggested approach is one candidate, not the answer.

**Make one of them radical success:** what this would be if it worked far
beyond the brief.

**Give each its cost.** A short title, one line on what is true in that world,
its demo (what someone runs and sees once its first end-to-end slice works),
and its cost and risk beside its appeal. The test: could someone pick one
without asking what it would take?

**Three is inspiration, not a rule.** A Task with one obvious answer says so
and spends the time on its open question.

## Make them tangible

**Show the thing, not a description of it.** Write `scratch/explore.page.html`
following `lf help page`, compile it with `lf page build scratch/explore.page.html`,
and open it with `lf page open scratch/explore.html` in conversation. Lead with the decision
needed and include a panel per vision: for anything people see or touch,
real-looking content and the key interaction; for other work, CLI transcripts,
data-model sketches, a day in the life of the system, or the failure each
avoids. Add `data-choice` / `data-question` controls for the real forks and
numbered questions at the end. Rebuild the same file each round.

## In conversation

**Answer each request as given.** The first page is an opening move, not a
ballot. The answer may be one outright, A + B, or "three more, keep X fixed
and vary Y", for many rounds. Don't steer toward closure, summarize a choice
not yet made, or turn a remark into a decision.

**Write the decision in the design, not the page.** Record what will be built
in the Markdown design, with the exact words people used and the speaker's
name. "Looks fine" stays "looks fine"; open threads stay open. After "build
this", write the chosen direction there for planning and link the latest page.

## Headless

Write the page and recommend one vision. In the Markdown design under
`scratch/`, name the recommended option, why it fits, and what planning
should build if nobody intervenes. Label it as your recommendation. A later
choice takes precedence.

In your final answer, link `scratch/explore.html`, summarize the tradeoff,
and repeat the recommendation and numbered choices so someone can answer
from the conversation.
