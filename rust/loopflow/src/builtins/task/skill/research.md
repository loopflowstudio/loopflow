---
requires: none
produces: a sourced answer, or a research note under scratch/, for the question asked
---
Map the territory. Understand what exists before deciding what to change.

Read `scratch/` first: `scratch/<branch>.md` is this PR's design and
`scratch/questions.md` holds open questions. Read Wave or Project context only
when the seed names the exact wave, Task or Project. Don't re-derive what these
already say.

## Scope

Answer the question asked, at the depth asked. A narrow codebase question gets a
direct sourced answer, not an architecture survey. Go broader only when the
decision depends on it. In conversation, follow new questions as they come;
headless, return the best answer the sources support. Don't implement, and don't
offer critique nobody asked for.

## Where to look

Follow the paths that matter to the question:

1. **Entry**: README, entry points, happy path. How does someone start using this?
2. **Architecture**: Main modules, data structures, key abstractions.
3. **Data flow**: How does information move through the system?
4. **Dependencies**: What does this area depend on? What depends on it?
5. **Patterns**: What conventions exist, and where do they break?
6. **Tests**: What's tested, and where are the gaps?

## Know what you know

- Keep what you saw separate from what you infer. A source, code path, trace or
  experiment backs a claim; resemblance does not.
- When doubt would change the answer, keep two different explanations alive and
  run the smallest safe check that tells them apart.
- Test your model against all the sources, especially ones that predate or
  contradict the favored explanation. A contradiction means the model is wrong,
  not that one claim needs a patch.
- For parallel research, split agents by independent approach and ask for
  concrete findings and exact gaps, not status. Don't share the favored answer
  until each pass has found its own gaps.

## Output

Answer here when the reader needs an explanation. For longer research, update
the existing note or write a topic-named note under `scratch/`. Show how the
pieces fit, with concrete examples of tensions, gaps or unused capabilities when
they bear on the question. Open questions are a valid result; don't force a
recommendation.

A later agent can reread the code and rerun the commands, so don't restate
them. Write down what can't be found again: what people said, word for word,
what a run showed at the time, and facts from outside the repository.

When feeding research to another agent writing the design, write to the path you
were given or a distinct topic path, and leave the design to that agent. Write
the note complete in one go; a path alone does not carry its contents to another
checkout. Keep sources attached to claims.

## The page

When someone is reviewing the answer, present it as a page and talk about what
they see there: the answer first, then the system map as a diagram, sources
linked to their claims, counterexamples and open questions. A one-line answer
needs no page. Keep any lasting record in Markdown.

- One self-contained `.html` under `scratch/`: inline CSS and script, no
  network, no build step. Escape code. Readable in light and dark and at narrow
  widths.
- Lead with the decision needed. End with numbered questions, so "2b" is a
  complete answer.
- Show the thing, not a description of it. Mark proposed behavior apart from
  existing behavior.
- One reading column, generous whitespace, no dashboard chrome or badges. Shape
  the structure to the content.
- Header: Task, step, date, and the files it is built from.
- Say its path. Loopflow Desktop shows `scratch/` pages beside the
  conversation; in a bare terminal, open it in the browser. Regenerate the same
  path when the conversation changes it, and write it last, after the Markdown.
