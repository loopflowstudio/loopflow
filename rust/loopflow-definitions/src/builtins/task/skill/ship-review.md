---
requires: a built change with a published PR, and what the run wrote about it
produces: one page that says what ships and what was left outside it, a decision on every finding, and where the work goes next
action_style: conversational
---
Make what ships understood, and decide what happens to everything the run left outside it.

The work is built and the PR is published. Concept review and gate own
whether the change is right. This is not a code review and not a go/no-go.

**Gather what exists before writing anything.**

- `scratch/plan.md`: what was to be built, what remains, choices agents made.
- `scratch/concept-review.html`: what the review found, and what it put
  outside this PR.
- What `qa` used and saw, and what it says is still wrong.
- `git log` against the base: each step's commit says what that step changed.
- `scratch/questions.md`: what was left open.
- The PR and its checks.

**Write the reading order.** When `scratch/tour.yaml` is missing or behind,
write it as `lf help tour` describes.

**Write `scratch/ship-review.html`.** One self-contained file, readable in
light and dark, in this order:

1. What ships, in the person's terms: what they will do differently and what
   they will see, with real output.
2. How to try it: the built thing at the place to start, the command ready to
   type or the window open.
3. What is likeliest to surprise them: choices made on their behalf, what was
   passed over, behavior that changed on purpose.
4. Findings: everything left outside the PR, each with a proposal. Fix here,
   its own Task, or drop.
5. Numbered questions, so "2b" is a complete answer.

Show the thing instead of describing it, and keep real code and real output
larger than the commentary. Say the page's path.

**Hold a high bar for a Task.** File one only for what is clearly important
and too big or too scope-expanding to fix here. A small thing is fixed here
or not filed, unless there is a real reason to separate it. Several such
findings go into one Task, not one each.

**The person decides.** Ask about what is likeliest to surprise them, never
"does this look good?". They may disagree that something belongs outside this
PR; then it is this work. File Tasks once they have said which. Don't change
the code or the plan until they say what they want.

**Keep their words.** Write what they said, word for word, in a note under
`scratch/`, apart from your reading of it. "Looks fine" stays "looks fine".
Going quiet or closing the window is never approval.

Finish with one outcome: build more, with their words written into the plan
for the next pass; or go on to delivery.

## With nobody there

Build the page all the same; it is the record they read afterwards. Fix small
things here: copy, a missed rename. File every untracked finding that clears
the bar, together where they belong together. Never reopen the design. Go
back to build only when a promised behavior is missing or broken. Otherwise
go on to delivery, and say on the page which questions are still theirs.
Don't invent a reaction.
