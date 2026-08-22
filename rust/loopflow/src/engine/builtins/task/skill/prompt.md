---
description: Author or audit a Loopflow skill, Wave goal, direction, or inline prompt.
requires: a prompt idea or existing prompt asset
produces: .lf/skills/*.md | .lf/directions/*.md | wave/<name>/GOAL.md | reviewed prompt text
default_agent: claude
action_style: exploratory
---
Turn intent into a prompt that can steer real work and prove when it is done.

```bash
lf prompt: create a dependency-audit skill
lf prompt: tighten wave/infra/GOAL.md
```

## Reviewer mode

The launch prompt identifies the reviewer for this exercise.

- **Human reviewer:** read any named file, state the contract you see, and ask
  only about choices whose answers materially change behavior, scope, or
  authority. Write reversible improvements as decisions land.
- **Parent reviewer:** treat the supplied directive, quoted user language, and
  existing asset as intent. Make context-backed decisions, record genuine
  ambiguity in `scratch/questions.md`, and use the review protocol to return the
  exact proposed content. Do not wait for an unavailable human or claim their
  confirmation.

## Choose the artifact

Put each instruction at the narrowest layer that exercises it:

| Artifact | Use it for | Do not put here |
| --- | --- | --- |
| Skill | A repeatable task and its output contract | Repo-wide conventions or Wave portfolio policy |
| Wave `GOAL.md` | Durable identity, bounds, cadence, and selection judgment | Project KRs, live metric contracts, task lists, implementation steps |
| Direction | A composable quality or user intent | A workflow tied to one skill or code area |
| Inline prompt | One concrete request | Reusable doctrine that deserves a skill |
| Repo agent doc | Conventions every task in this repository must follow | One feature's design or temporary context |

Do not repeat Loopflow's ambient operating guidance in customer prompts. It is
already supplied to standard runs. Add only the domain contract and method this
artifact uniquely owns.

## Workflow

1. **Resolve the target.** Infer the artifact kind from the named path or
   request. Read the existing file when present. If a human request leaves two
   materially different targets possible, ask one focused question; otherwise
   choose the narrower artifact and proceed.

2. **Write the contract.** Make these computable:
   - exact task or durable objective;
   - observable success;
   - plausible near-misses that do not count;
   - affected boundaries, edge cases, permissions, and exclusions;
   - the command, observation, or artifact that proves success.

3. **Design the evidence loop.** For uncertain work, tell the agent to preserve
   observations separately from hypotheses, externalize the cheapest useful
   model, run the smallest safe check that separates leading explanations,
   verify against all relevant old and new evidence, and replan after a
   counterexample. Ask tools or tests to enforce this when prose cannot.

4. **Write the prompt.** Start directly. Use imperative language, concrete
   verbs, and only the sections the artifact needs. Put output shape next to
   the work that produces it. Include examples when they carry more information
   than explanation.

5. **Audit adversarially.** Read the candidate as an agent trying to finish
   cheaply. Could it satisfy the words while missing the intent? Does a receipt
   such as “tests added” masquerade as the outcome? Does it know what to do when
   evidence contradicts the favored plan? Tighten the contract until the easy
   loopholes close.

6. **Deliver at the source.** Update the named customer file or return reviewed
   prompt text. Do not create a second copy in documentation. Summarize the
   behavioral change, not each wording edit.

## Skill contract

Place repo-local skills under `.lf/skills/`. Use frontmatter for machine
configuration, then one direct opening line:

```markdown
---
requires: diff vs main
produces: scratch/security-audit.md
---
Audit the changed authentication boundary and prove every caller still fails closed.

## Workflow

1. Reconstruct the affected trust boundary.
2. Exercise the success, denial, absent-credential, and malformed-input paths.
3. Write only reproducible findings.

## Output

Write `scratch/security-audit.md` with evidence, severity, and reproduction.
```

Give procedural skills numbered work and a concrete output. Give exploratory
skills room to follow evidence without turning “explore” into permission to
change unrelated code. Skills that may run with a present human must also
define bounded behavior for a headless parent reviewer.

## Wave goal contract

The body of `wave/<name>/GOAL.md` is the prompt a Wave runs repeatedly. Make it
loop well:

1. **Identity by contrast** — what this Wave owns and what a sibling owns.
2. **Selection signals** — the evidence that changes Project selection or
   strategy. Reference Project-owned metrics when they exist; never copy them
   into the Wave body.
3. **Concrete moves** — the kinds of useful action it may select now.
4. **Honest question** — the check a lazy loop cannot satisfy by gaming a proxy.
5. **Stop discipline** — when to record a blocker instead of manufacturing
   work.

Frontmatter carries machine policy such as `agent`, `crons`, `pm`, and `home`.
Keep Project definitions and proof-shaped KRs in the Project system. Keep
concrete implementation in Tasks. A Wave chooses among measured bets; it does
not contain a roadmap disguised as a prompt.

## Direction contract

A direction changes judgment without prescribing steps:

```markdown
Make operational failure obvious before it becomes expensive.

- Can an operator see the failing boundary without opening raw logs?
- Does the signal identify the next owner and safe next action?
- Will retries preserve the evidence needed to explain the first failure?
```

Keep directions orthogonal to skills and code areas. “When reviewing this API”
is a coupled workflow; “make operational failure obvious” composes with design,
implementation, review, and any area.

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
- Output is useful to the next human or agent.
- Repeated runs can stop without inventing work.
