Generate a PR title and body for the changes on this branch.

Read the diff against the PR base. Don't ask questions; assume from context.

## Writing it

Write for someone coming back after time away. The title names the concrete
improvement. The body opens with one or two sentences: what was hard before,
and what this PR makes easier or newly possible. A reader should get the
benefit without running a command or opening another document.

Promise only what this PR delivers, even when the Task aims further. Use
product language and concrete verbs. Add an area prefix only when it helps
recognition. Keep proper names and command spelling exact.

Then **What changes**: a short paragraph or a few bullets. Add implementation
detail only when it helps review. **Why it matters** comes next, only if the
opening doesn't already say it. Note real risks or limits. Keep checks that
can only run after merge: what is still untried, the environment
or release it needs, and what to look for. A later Task may be created from
this text after `scratch/` is cleared; mark new-scope ideas as suggestions.
Test, lint and QA results never go in the body; CI shows them. End with **Try it** when
a walkthrough helps: a concrete action and the visible result. Tests never go
in **Try it**. Say which steps were actually run and which are suggested.

Don't restate what the diff already says; a reader can open it. Spend the
words on what it can't show: why it was built this way, quoted direction, what
a run showed, outside facts. Use only the sections the
change needs. When scope changes, rewrite around the
current diff; no diary of revisions. Loopflow adds the Task ID and merge
consequences itself; don't repeat them.

Link related work where you explain why it matters:
`[Title · Task ID or PR number](known URL)` on first mention, shorter after.
Say how it relates: builds on, supersedes, verified by. Only use URLs you
know. Decisions you cite must stay readable after merge. In operational lists,
ID first: `[Task ID or PR number · Title](known URL)`.

## Output format

Return a structured response with **title** and **body** (Markdown).

Example title: `Switch Tasks without losing unfinished input`

Example body (illustrative; adapt the walkthrough to the actual change):

```markdown
Switching Tasks used to lose unfinished terminal input. Returning to a Task
now restores its terminals and draft input so you can pick up where you left off.

## What changes

Each Task retains its terminal layout and unfinished input while you visit
other work.

## Try it

Type an unfinished command in a Task terminal, switch Tasks, then return.
Your original terminal and unfinished input should still be present.
This is a suggested walkthrough; it has not been performed for this example.
```
