---
requires: a built change with a published PR, and what the run wrote about it
produces: one page that says what ships and what was left outside it, proposed finding dispositions, and a recommended next action
action_style: conversational
---
Make what ships understood, and decide what happens to everything the run left outside it.

The work is built and the PR is published. Concept review and CI own
whether the change is right. This is not a code review and not a go/no-go.

**Gather what exists before writing anything.**

- `scratch/plan.md`: what was to be built, what remains, choices agents made,
  and `## Deferred findings` written by `realign`.
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
and open it with `lf page open scratch/ship-review.html` in conversation.

**Hold a high bar for a Task.** File one only for what is clearly important
and too big or too scope-expanding to fix here. A small thing is fixed here
or not filed, unless there is a real reason to separate it. Several such
findings go into one Task, not one each.

## In conversation

**The person decides.** Ask about what is likeliest to surprise them, never
"does this look good?". They may disagree that something belongs outside this
PR; then it is this work. File Tasks once they have said which. Don't change
the code or the plan until they say what they want.

**Keep their words.** Write what they said, word for word, in a note under
`scratch/`, apart from your reading of it. "Looks fine" stays "looks fine".
Going quiet or closing the window is never approval.

Finish with one outcome: build more, with their words written into the plan
for the next pass; or go on to delivery.

## Headless

Write `scratch/ship-review.html` and `scratch/tour.yaml`. Never reopen the
design, merge the PR, complete the Task or file optional follow-up Tasks ahead
of review; propose each finding's disposition (fix here, its own Task or drop)
and leave merge, completion and optional follow-ups to the step or conversation
that owns them.

Recommend returning to build (`iterate`) only when a promised behavior is
missing or broken; otherwise recommend advancing to delivery (`advance`). In
your final answer (and loop verdict summary), link `scratch/ship-review.html`
and `scratch/tour.yaml`, summarize what ships and what was left outside it,
state the recommended next action and repeat the numbered choices so someone
can answer from the conversation. Don't invent a reaction.
