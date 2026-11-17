---
requires: a built change with a published PR, and what the run wrote about it
produces: one page a person can judge the change from, their direction in their words, and where the work goes next
action_style: conversational
---
Put everything the run learned about this change in front of the person, on one page, and take their direction.

The work is built and the PR is published. Agents have already planned it,
reviewed its concepts, used it and written down what they saw. Your job is to
make that easy to judge, not to review it again.

**Gather what exists before writing anything.**

- `scratch/plan.md`: what was to be built, what remains, choices agents made.
- `scratch/concept-review.html` and the review's findings in the plan: do the
  product's concepts and the code's concepts line up?
- What `qa` used and saw, and what it says is still wrong.
- `git log` against the base: each step's commit says what that step changed.
- `scratch/questions.md`: what was left open.
- The PR and its checks.

**Fill a gap; don't repeat a finished step.** When a piece is missing or
describes an older state of the branch, do that piece now: review the concepts,
or use the change through its real path. When it is there and current, quote
it.

**Write `scratch/human-review.html`.** One self-contained file, readable in
light and dark, in this order:

1. What changes for the people who use it, in a few lines.
2. Product concepts beside code concepts, each pairing marked yes or no, and
   the biggest mismatch with what it would take to fix.
3. How to try it: the built thing at the place to start, the command ready to
   type or the window open.
4. What was checked and what it showed. What was not checked, and why.
5. Choices made on the person's behalf: what was chosen and what was passed
   over.
6. Numbered questions, so "2b" is a complete answer.

Show the thing instead of describing it, and keep real code and real output
larger than the commentary. Say the page's path. When a page already exists
for the branch exactly as it stands, open with that one.

**Then get out of the way.** Answer questions and follow where they go. Ask
about choices that change what the product should be, never "does this look
good?". Don't change the code or the plan until they say what they want.

**Keep their words.** Write what they said, word for word, in a note under
`scratch/`, apart from your reading of it. "Looks fine" stays "looks fine".
Going quiet or closing the window is never approval.

Finish with one outcome: build more, with their words written into the plan
for the next pass; or go on to delivery.

## With nobody there

Stand in only for what can be judged without them. Build the page all the
same; it is the record they read afterwards. Go back to build when a promised
behavior is missing or broken. Otherwise go on, and say on the page which
questions are still theirs. Don't invent a reaction.
