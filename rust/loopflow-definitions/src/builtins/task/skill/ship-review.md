---
requires: a built change with a published PR, and what the run wrote about it
produces: one page that says what ships and what was left outside it, a decision on every finding, and where the work goes next
action_style: conversational
---
Make what ships understood, and decide what happens to everything the run left outside it.

The work is built and the PR is published. Concept review and CI own
whether the change is right. This is not a code review and not a go/no-go.

**Gather what exists before writing anything.**

- `scratch/plan.md`: what was to be built, what remains, choices agents made,
  and `## Deferred findings`.
- `scratch/concept-review.html`: the review page behind those findings.
- What `qa` used and saw, and what it says is still wrong.
- `git log` against the base: each step's commit says what that step changed.
- `scratch/questions.md`: what was left open.
- The PR and its checks.

**Write the reading order.** When `scratch/tour.yaml` is missing or behind,
write it as `lf help tour` describes.

**Write `scratch/ship-review.page.html` following `lf help page`**, in this
order:

1. **What was built**, in the person's terms: what they will do differently and
   what they will see, with real output. Put "How to try it" as one line with
   the command ready to run.
2. **How it works** on real hunks (`<lf-diff>` with `<lf-note>` where helpful,
   `<lf-file>` for unchanged context).
3. **Key data structures and APIs**: the real types, schemas, wire formats, or
   storage tables someone building on top of this needs to know.
4. **Trade-offs and invariants going forward**: choices made on their behalf,
   what was passed over, behavior that changed on purpose, and what must hold.
   When the diff touches security-relevant code—auth, credentials or secrets,
   telemetry or outbound data, untrusted input, or deployment—call out what
   changed and suggest running `lf security-review` before delivering.
5. **Findings**: everything left outside the PR, each as a `decision` component
   (`<lf-component name="decision" id="…" title="…" recommendation="…">`) for
   Fix here, File Task, or Drop. Add a `data-lf` action button only when a
   direct operation (like filing a follow-up Task now) is clearly useful.
6. **Numbered questions**, so "2b" is a complete answer.

Omit passing test counts and routine CI checks (`land --wait-and-fix` handles
routine failures). Compile with `lf page build scratch/ship-review.page.html`
and present with `lf page open scratch/ship-review.html`.

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
