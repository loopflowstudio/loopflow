---
description: Author or audit a Loopflow skill, Wave goal, or inline prompt.
requires: a prompt idea or existing prompt asset
produces: .lf/skills/*.md | wave/<name>/GOAL.md | reviewed prompt text
default_agent: claude
action_style: exploratory
---
Turn intent into a prompt that can steer real work and prove when it is done.

```bash
lf prompt: create a dependency-audit skill
lf prompt: tighten wave/infra/GOAL.md
```

Read the supplied intent and existing asset. When someone is present, ask only
about choices that materially change behavior, scope, or authority, and edit as
decisions land. Otherwise make evidence-backed corrections and leave genuine
ambiguities visible without inventing confirmation. Use a review protocol when
the caller supplies one; ordinary authoring needs no reviewer role or protocol.

## Choose the artifact

Put each instruction at the narrowest layer that exercises it:

| Artifact | Use it for | Do not put here |
| --- | --- | --- |
| Skill | A repeatable task and its output contract | Repo-wide conventions or Wave portfolio policy |
| Wave `GOAL.md` | Durable identity, bounds, cadence, and selection judgment | Chapter KRs, live metric contracts, task lists, implementation steps |
| Inline prompt | One concrete request | Reusable doctrine that deserves a skill |
| Repo agent doc | Conventions every task in this repository must follow | One feature's design or temporary context |
| Wave `MEMORY.md` | Curated decisions, lessons, and evidence limits | A second plan or a transcript |

Do not repeat Loopflow's ambient operating guidance in customer prompts. It is
already supplied to standard runs. Add only the domain contract and method this
artifact uniquely owns.

Shared skills must work in a customer's repository without your source files,
internal issue IDs, tool wrappers or secret-manager policy. Keep specialized
procedures in the skills that perform them. Repo-local skills replace the named
skill; they are not appended supplements or automatically exported to customers.
Improve the shared skill for general lessons; keep local rules with their local
consumer. Do not create miscellaneous `.lf/` learning notes.

Generate only headless, machine-runnable checks. Implement/compress build and
run focused sanity tests; gate owns acceptance and affected suites once. An
unavailable check goes to capable gate/CI and does not block earlier work.
People's judgment belongs to demo/review, never automated acceptance. Scratch
keeps one command/result line, not an evidence ledger.

## Workflow

1. **Resolve the target.** Infer the artifact kind from the named path or
   request. Read the existing file when present. If a user request leaves two
   materially different targets possible, ask one focused question; otherwise
   choose the narrower artifact and proceed.

2. **Write the contract.** Make these computable:
   - exact task or durable objective;
   - observable success;
   - plausible near-misses that do not count;
   - affected boundaries, edge cases, permissions, and exclusions;
   - the command, observation, or artifact that proves success.

   Build a useful atom before choosing its place in a process. Require the
   information the task needs, not a preceding skill, named Flow, Task binding,
   branch name, or document layout that happened to supply it once. Keep real
   domain constraints and authority boundaries. Let callers compose synchronization,
   publication, and navigation around the operation.

   Match proof to maturity: an early Task needs a concrete problem and
   recognizable success; design chooses the implementation and headless acceptance checks at gate.
   Name the output's reader and their next decision. State how separate input
   artifacts reach the consumer's execution context and which copy stays current;
   a path alone does not deliver contents. Name a missing transfer mechanism
   instead of using a Task description as a substitute document store.

3. **Design the evidence loop.** For uncertain work, tell the agent to preserve
   observations separately from hypotheses, externalize the cheapest useful
   model, run the smallest safe check that separates leading explanations,
   verify against all relevant old and new evidence, and replan after a
   counterexample. Use that understanding to update the artifact that owns the
   truth while the context is available. Delete stale instructions; retain
   accepted constraints and unresolved contrary evidence. Save consequential
   rationale and a one-line check result. Leave a minimal review when it helps the next
   reader: progress against the plan, what worked, what was learned, and what
   remains unresolved. Supply evidence for the next decision without making
   that decision for its owner. Avoid duplicating the plan or accumulating
   reports by default. Ask tools or tests
   to enforce this when prose cannot.

4. **Write the prompt.** Start directly. Use imperative language, concrete
   verbs, and only the sections the artifact needs. Put output shape next to
   the work that produces it. Include examples when they carry more information
   than explanation.

5. **Audit adversarially.** Read the candidate as an agent trying to finish
   cheaply. Could it satisfy the words while missing the intent? Does a receipt
   such as “tests added” masquerade as the outcome? Does it know what to do when
   evidence contradicts the favored plan? Tighten the contract until the easy
   loopholes close. Check a real example: can someone without the transcript
   understand the benefit and current problem before opening tools? Keep the
   Task's problem distinct from the design's solution. Trace competing rules
   through examples, handoffs, later edits, and mechanical rendering. For Task
   authors, check the description with comments collapsed: the current problem,
   desired outcome, acceptance, and real blockers must still make sense. Dated
   planning/execution updates belong in authorized comments; raw receipts belong
   behind links. Test a later scope change: reconcile the brief instead of stacking
   amendments, preserving the decision history and unresolved contrary evidence.
   Apply this second-revision check to other artifacts too. Try the skill
   standalone, midstream with partial evidence, and again when the work already
   satisfies its contract. It should adapt, name exact missing inputs, and stop
   without manufacturing edits or reports. Put resulting guidance only in
   authorship surfaces that exercise it.

6. **Deliver at the source.** Update the named customer file or return reviewed
   prompt text. Do not create a second copy in documentation. Summarize the
   behavioral change, not each wording edit.
   When relocating instructions, inspect both the ordinary assembled prompt
   and the receiving skill's standalone export. Prove the old audience no longer
   pays for the procedure and the intended consumer can still execute it.

## Persisted plans

Plan-writing skills must produce step-neutral artifacts: named, dated decisions,
explicit acceptance or draft status, remaining work, and a one-line check result. Keep session/step
instructions and ambient Home facts out of plans. Historical skill invocations
use plain names without dollar prefixes; verbatim transcripts remain separate
reference evidence. Audit a plan as input to a different skill in a fresh Run:
the plan cannot select that reader's skill or claim its execution environment.

## Skill contract

Place repo-local skills under `.lf/skills/`. Use frontmatter for machine
configuration, then one direct opening line:

```markdown
---
requires: findings and the intended behavior
produces: corrected and prioritized findings in their existing location
---
Turn findings into an accurate, ordered set of remaining work.

## Workflow

1. Check the supplied findings against current behavior and evidence.
2. Remove duplicates and disproved claims; retain uncertain findings as uncertain.
3. Edit the original list with blockers first, evidence, and the next useful action.

Return conversation-only findings here. Do not create a second assessment file.
```

Give procedural skills numbered work and a concrete output. Give exploratory
skills room to follow evidence without turning “explore” into permission to
change unrelated code. Define behavior when required judgment is unavailable
without assuming a particular launch mode or reviewer protocol.

## Wave goal contract

The body of `wave/<name>/GOAL.md` is the prompt a Wave runs repeatedly. Make it
loop well:

1. **Identity by contrast** — what this Wave owns and what a sibling owns.
2. **Selection signals** — the evidence that changes chapter strategy or
   strategy. Reference Wave-owned metrics when they exist; never copy them
   into the Wave body.
3. **Concrete moves** — the kinds of useful action it may select now.
4. **Honest question** — the check a lazy loop cannot satisfy by gaming a proxy.
5. **Stop discipline** — when to record a blocker instead of manufacturing
   work.

Frontmatter carries machine policy such as `agent`, `crons`, `pm`, and `home`.
Keep current chapter metric targets and proof-shaped KRs in the internal Project, edited with `lf wave update-plan`. Keep
concrete implementation in Tasks. A Wave steers one current chapter; it does
not contain a roadmap disguised as a prompt.

## Parallel search

Use parallel approaches only when the task is genuinely uncertain, safely
divisible, and delegation is already authorized. Start with different
mechanisms, preserve early independence, keep a registry of evidence and exact
gaps, block routes whose missing dependency merely restates the original
problem, and require concrete artifacts or counterexamples. Cross-pollinate
after each route has exposed its own failure mode. Do not add multi-agent
ceremony to ordinary deterministic work.

## Final check

- The opening line says what to do.
- The artifact owns these instructions; no narrower layer should carry them.
- Success, insufficiency, boundaries, and proof are explicit where they matter.
- Observations cannot be silently rewritten to save a hypothesis.
- Unexpected evidence has a named consequence.
- Output is useful to the next reader or agent.
- The operation works outside its original process and edits the owning artifact.
- Reports have a distinct purpose; no-op runs need no new artifact.
- Repeated runs can stop without inventing work.
