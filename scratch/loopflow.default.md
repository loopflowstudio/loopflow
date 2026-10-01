# A responsive default conversation

Draft — 2026-10-01. Jack Heart requested: “make lf less of an opinonated default” because “the Desktop app + repo/wave agents cover a lot of the lf operational ground.” Earlier direction: “Prioritize responsiveness” and a “possible request is unrelated.”

## What to build

Bare `lf` opens a general-purpose conversation that follows the participant's direction and brings in Loopflow coordination when useful.

Jack Heart proposed merging the operational skills under `lf operate` on
2026-10-01. This supersedes the earlier proposal to retain `lf loopflow`.
The existing review skill is named `review-open-work` in the repository.

## Placement

Unresolved; no Wave was supplied. This bounded default-launch change needs no planning-provider lookup to design.

## Proposed exact default prompt

```markdown
---
description: Follow the conversation and help with whatever comes next.
---

Be ready to adapt to whatever the person in this conversation asks you to do.
Prioritize responsiveness: answer questions directly, follow changes in
direction, and do requested work in this checkout. The request may have
nothing to do with this repository or Loopflow.

Use Loopflow coordination when the work benefits from it—for example, a
substantial effort with independent workstreams, ongoing tracking, or an
explicit request to operate Tasks, Waves, or Sessions. Load the `operate`
skill when that operational guidance is needed, within the user's existing
authorization.

Let the conversation set the agenda. Do not begin with a Session inventory,
Task reconciliation, or planning lookup unless the request calls for it.
If no request has been supplied, briefly invite one and wait.
```

## Current system

`run_default_agent` in `rust/loopflow/src/bin/lf.rs` selects the `loopflow`
skill in both worktree-placement branches. That skill requires
`lf session list --json` on every turn and forbids requested interventions
in the current checkout. It makes even unrelated conversation operational.

Launch preparation and `exec_prompt` in
`rust/loopflow/src/lf/commands/run.rs` special-case the `loopflow` skill for
terminal launch. `bare_lf_has_a_terminal_control_skill` in the binary's tests
asserts the old agenda. The shared `LOOPFLOW.md` already says to execute here
first and use orchestration only when requested or called for by a skill.

Repo and Wave conversations have their own skills and owners. Their ongoing
operational responsibilities remain appropriate to those explicit contexts.

## Data structures and key functions

Reuse the existing skill registry and prompt assembly; no new domain values,
configuration, or persistence. Add `ops/skill/default.md` with the exact text
above. `run_default_agent(cli: &Cli, command: &[String]) -> anyhow::Result<()>`
selects `default`. Preserve terminal launch for this entry path through the
existing launch-target handling; assemble and execute the same surface.

## Explicit operation

Create `ops/skill/operate.md` by consolidating `loopflow` and
`review-open-work`. Its purpose: inspect current work, identify what needs
attention, and carry authorized actions through to an observable result.

With a specific request, inspect only the relevant work. With no request,
review the repository's open work and present a short triage: what is running,
waiting for a decision, ready to ship, stale, or blocked. Expand to branch,
PR, and Wave review as needed. Refresh relevant facts before acting, rather
than inventorying every Session on every turn.

Retain Session connection and review handling, existing Task/Flow identity,
design handoff, launch, restart, placement, and execution diagnosis from
`loopflow`. Retain clear/ship/abandon/prune triage and progress-against-purpose
review from `review-open-work`. Use shared state readers and existing delivery
skills. Replace the review skill's direct branch deletion and unconditional
background shipping recipes with supported `lf` operations and existing
authorization boundaries. Missing provider data stays unknown.

This is an invoked operational conversation, not another persistent repo or
Wave agent. Do local work directly when appropriate; respect existing workers
and managed Flow authority. Keep decisions with the participant here. Destructive
cleanup and external mutations still require applicable authorization.

## Constraints and forbidden outcomes

- Preserve bare `lf` worktree placement and terminal presentation. Changing
  either is outside this request.
- Keep repo and Wave agents' responsibilities intact. `lf operate` is the
  explicit operational entry point; `wave/operate` remains Wave-specific.
- Do not copy the control skill's procedures into the default prompt.
- Do not make unrelated requests trigger PM access, worker launch, or a
  requirement to fit the request into a Task.
- Substantial work makes coordination useful; it does not grant new authority
  to launch workers or publish changes.

## Delete — do not maintain

Remove the default launch's selection of the control skill and replace
`bare_lf_has_a_terminal_control_skill` with coverage of the new default
selection. Remove comments describing bare `lf` as terminal control.
Delete `ops/skill/loopflow.md` and `wave/skill/review-open-work.md` after their
useful behavior moves to `operate.md`. Update catalog, documentation, Flow,
and test references together, including old skill-name launch special cases.
Do not retain aliases, duplicate procedures, or tests asserting removed names.

## Internal slices

One indivisible change, shipped as one PR through internal slices.

**This slice:** consolidate the two operational skills into `operate`, remove
the superseded skills, and cut over their consumers. Add the short default
skill and switch bare launch in the same cut so no launch target disappears.

Then finish terminal-routing coverage, prompt-ownership coverage, and examples
contrasting `lf` with `lf operate`. No follow-up Tasks needed.

## The demo

Open `lf`: a short invitation, no operational inventory. Ask an unrelated
question: receive a direct answer. Ask for a small local edit: work proceeds
here. Describe a substantial effort: coordination becomes available in
response to its needs. `lf operate` offers open-work triage; `lf operate` with
a specific request goes directly to the relevant work.

## Done when

Headless gate: `cargo test -p loopflow --bin lf bare_lf` verifies default
selection and terminal routing without launching a provider. Extend
`assembled_prompts_deliver_procedures_to_the_owning_skill` to include `default`
and replace `loopflow` with `operate`;
`cargo test -p loopflow assembled_prompts_deliver_procedures_to_the_owning_skill`
passes with operational procedures confined to their owning skills.
Add discovery coverage proving `operate` resolves distinctly from
`wave/operate`; run `cargo test -p loopflow lf::discovery`.
`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.
Conversational responsiveness is judged in demo/review, not by brittle prose
assertions or a live-provider gate.

Check: source inspection only; design has no executable changes.
