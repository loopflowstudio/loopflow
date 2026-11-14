---
description: Write or audit a Loopflow skill, Wave goal, or inline prompt.
requires: a prompt idea or existing prompt
produces: .lf/skills/*.md | .lf/wave/<name>/GOAL.md | reviewed prompt text
action_style: exploratory
---
Turn intent into a prompt that steers real work and knows when it is done.

```bash
lf prompt: create a dependency-audit skill
lf prompt: tighten .lf/wave/infra/GOAL.md
```

Read the intent and the existing file. In a conversation, ask only about
choices that change behavior or scope, and edit as decisions land. Headless,
make the corrections you can back up and leave real ambiguity visible. Use a
review protocol only when the caller supplies one.

## Pick the layer

Put each instruction at the narrowest layer that uses it:

| Layer | Use it for | Not for |
| --- | --- | --- |
| Skill | A repeatable task and its output | Repo-wide conventions or Wave policy |
| Wave `GOAL.md` | What the Wave owns, its bounds, cadence and judgment | Chapter KRs, metric targets, task lists, implementation steps |
| Inline prompt | One concrete request | Guidance that deserves a skill |
| `AGENTS.md` | Conventions every task in the repo follows | One feature's design or temporary context |
| Wave `MEMORY.md` | Curated decisions and lessons | A second plan or a transcript |

Don't repeat Loopflow's standard operating guidance; every run already gets it.

Shared skills run in other people's repositories: no source paths, internal
issue IDs, tool wrappers or secret-manager policy. A repo-local skill in
`.lf/skills/<name>.md` replaces the builtin of the same name; it doesn't append
to it. Put general lessons in the shared skill and local rules in the local one.
No miscellaneous `.lf/` learning notes.

Checks are headless and machine-runnable. Implement and compress run focused
tests; gate starts CI for the suites and selects a few high-risk tests, including
acceptance. Unavailable checks go to capable CI. Judgment belongs to demo and
review. Keep one command/result line in scratch, or the PR body after cleanup.

## Workflow

1. **Find the target.** Infer the layer from the path or request and read the
   file. If two very different targets fit, ask one question; headless, pick
   the narrower one.

2. **Define done.** Spell out:
   - the exact task or standing objective;
   - what success looks like when someone uses it;
   - near-misses that don't count;
   - edge cases and exclusions;
   - the command or observation that shows it worked.

   Make the skill useful on its own before fitting it into a Graph. Ask for the
   information the task needs, not the preceding skill, Graph, branch name or
   document layout that happened to supply it once. Callers handle sync,
   publication and navigation around it.

   Match the bar to the stage: an early Task needs a concrete problem and
   recognizable success; design picks the implementation and the gate checks.
   Name who reads the output and what they decide next. If inputs live in
   separate files, say how their contents reach the agent; a path alone
   delivers nothing.

3. **Build the feedback loop.** For uncertain work, tell the agent to keep
   observations apart from hypotheses, run the smallest safe check that
   separates the leading explanations, test against old and new findings, and
   replan after a counterexample. Update the file that holds the truth while
   the context is fresh, delete stale instructions, and keep one check-result
   line. Skills that write notes should keep only what a later agent can't
   work out again from code, git or a fresh run: what people said, word for
   word, what a run showed, outside facts. Never turn a reply into a verdict:
   "looks fine" stays "looks fine", not "accepted". When it helps the next reader, leave a short review: progress against
   the plan, what worked, what was learned, what's open. Don't make their
   decision for them. Use tools or tests where prose can't hold the line.

4. **Write it.** Start directly. Imperative, concrete verbs, only the sections
   it needs. Put output shape next to the work that produces it. Use an example
   when it says more than explanation.

5. **Audit it as a lazy agent.** Could it satisfy the words and miss the point?
   Does "tests added" pass for the outcome? Does it know what to do when
   findings contradict the plan? Close the easy loopholes.

   Try a real example: can someone without the transcript see the benefit and
   the current problem? Keep the Task's problem apart from the design's
   solution. For Task descriptions, read with comments collapsed: problem,
   outcome, acceptance and real blockers must still make sense; dated updates
   go in comments. Change the scope once and check the brief is reconciled, not
   stacked with amendments. Run the skill standalone, midway with partial
   input, and on work that's already done: it should adapt, name exactly what's
   missing, and stop without manufacturing edits or reports.

6. **Edit at the source.** Update the named file or return the prompt text; no
   second copy in docs. Summarize the change in behavior, not each wording
   edit. When moving instructions, read both the assembled prompt and the
   receiving skill's standalone export: the old audience no longer pays for it
   and the new one can still follow it.

## Plans

Skills that write plans produce plans any later step can read: what will be
built, remaining work and one check-result line, with quotes showing what was
said about it. A skill that needs a real answer asks a specific numbered
question so "2b" is a complete answer, not "is this plan good?". No session or step instructions, no Machine facts. Write past skill
invocations without `$`; transcripts stay separate. Read the plan as a
different skill in a fresh Session would: it can't choose that skill or
assume its environment.

## Skill shape

Frontmatter for machine configuration, then one direct opening line:

```markdown
---
requires: findings and the intended behavior
produces: corrected and prioritized findings in their existing location
---
Turn findings into an accurate, ordered set of remaining work.

## Workflow

1. Check each finding against current behavior.
2. Remove duplicates and disproved claims; mark uncertain ones uncertain.
3. Edit the original list: blockers first, then the next useful action.

Return conversation-only findings here. Don't create a second file.
```

Procedural skills get numbered steps and a concrete output. Exploratory skills
get room to follow what they find, scoped to the task. Say what to do when a
needed judgment isn't available, without assuming a launch mode.

## Wave goals

The body of `.lf/wave/<name>/GOAL.md` is the prompt a Wave runs over and over.
Make it loop well:

1. **Identity by contrast** — what this Wave owns and what a sibling owns.
2. **Selection signals** — what changes chapter strategy. Point to the Wave's
   metrics; don't copy them in.
3. **Concrete moves** — the kinds of action it can take now.
4. **Honest question** — the check a lazy loop can't game.
5. **Stop discipline** — when to record a blocker instead of making up work.

Frontmatter holds `agent`, `pm`, `home` and other machine settings. Chapter
metric targets and KRs live in the Project, edited with `lf update-plan`.
Implementation lives in Tasks. A Wave steers one chapter; it isn't a roadmap.

## Parallel search

Use parallel approaches only when the work is genuinely uncertain and splits
cleanly. Start each on a different mechanism, keep them independent early,
track what each found and still lacks, block routes that just restate the
original problem, and ask for working code or counterexamples. Cross-pollinate
once each has hit its own failure mode. Ordinary deterministic work doesn't
need it.

## Final check

- The opening line says what to do.
- No narrower layer should carry these instructions.
- Success, near-misses and how to tell are clear where they matter.
- Observations can't be quietly rewritten to save a hypothesis.
- A surprising finding has a named consequence.
- The output helps the next reader.
- It works outside its original process and edits the file it belongs to.
- Repeated runs stop when there's nothing to do.
