Generate a PR title and body for the changes on this branch.

Read the diff against the PR base. Don't ask questions; assume from context.

Write for someone coming back after time away. The title names the concrete
improvement: "Switch Tasks without losing unfinished input", not "Persist
terminal state per Task". The body opens with one or two sentences on what was
hard before and what this PR makes easier or possible, so a reader gets the
benefit without running a command or opening another document.

Promise only what this PR delivers, even when the Task aims further. Product
language, concrete verbs, exact command spelling.

Then **What changes**, in a short paragraph or a few bullets. The diff shows
what the code does; spend the words on what it can't show: why it was built
this way, what people said that shaped it, what a run showed. Note real risks
and limits. Use only the sections the change needs, and when scope changes,
rewrite around the current diff rather than keeping a diary.

Test, lint and QA results never go in the body; CI shows them. A check that
can only run after merge stays: what is untried, the environment it needs and
what to look for. End with **Try it** when a walkthrough helps: a concrete
action and what appears, never a test command. Say which steps were actually
run.

Link related work where you explain why it matters, as
`[Title · Task ID or PR number](URL)`, with the relationship: builds on,
supersedes, verified by. Only URLs you know. Loopflow adds the Task ID itself.

## Output format

Return a structured response with **title** and **body** (Markdown).

```markdown
Switching Tasks used to lose unfinished terminal input. Returning to a Task
now restores its terminals and draft input so you can pick up where you left off.

## What changes

Each Task keeps its terminal layout and unfinished input while you visit
other work.

## Try it

Type an unfinished command in a Task terminal, switch Tasks, then return.
Your terminal and unfinished input should still be there. (Suggested; not run.)
```

## Command changes

`lf help --all` prints the public command tree. When the diff adds, removes or
renames a Clap command, compare that tree before and after the change and list
each added or removed command under **What changes**: "Adds `lf skill customize`",
"Removes `lf transport start`". Mention changed arguments only when they change
what someone types. For an added command, name the Task or design it delivers,
or say none was recorded. Keep this list even when the change looks minor.
