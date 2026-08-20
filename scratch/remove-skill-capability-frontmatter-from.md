# Remove Skill Capability Frontmatter From Task Validation

## Problem

Task lifecycle validation currently decides whether a delivery Task can launch by
looking for `capabilities: [task_implementation]` on a skill in its first or loop
flow. That check adds Task-specific metadata to every custom implementation skill
without proving that the skill will implement anything: an implementation prompt
without the label is rejected, while a non-implementing prompt with the label is
accepted.

This cuts against the Loopflow API project's endurance KR, which explicitly needs
real work to run with "zero repo-authored skills added for the work." Repositories
should be able to compose their existing skills into a Task lifecycle. Loopflow
should validate properties it can observe from the expanded flow and reserve
delivery judgment for the branch, review, gate, CI, and settlement evidence that
exists after execution.

## The demo

Create a repo-local implementation skill with plain Markdown and select a loop
flow containing it for a new Task. `lf task run DES-126 --loop custom-loop`
accepts the lifecycle without a capability declaration; a human-only loop or a
final flow that does not end in `op: pr land -c` is still rejected with the
existing actionable errors.

## Approach

Remove `capabilities` from the skill model and frontmatter parser, delete the two
built-in `task_implementation` declarations, and remove the implementation-label
branch from `validate_task_lifecycle_facts`. A skill remains a prompt plus launch
configuration (`agent`, `default_agent`, `directions`, and `action_style`), not a
self-attested Task outcome.

Keep the lifecycle checks backed by the expanded graph:

- every phase must resolve to a non-empty flow with phase-legal step kinds;
- the loop must contain autonomous agent work rather than only human nodes;
- the terminal finally step must be `op: pr land -c` (or `--complete`).

Preserve `TaskOutcome`, including `delivery`, `design_only`, `--design-only`,
storage, and status output. It is the Task's explicit durable intent and is wider
than this cleanup. It must no longer select a skill-frontmatter exception or be
presented as pre-execution proof that a prompt will produce an implementation.

Update the authoring and Task lifecycle docs to describe the remaining structural
contract and the evidence boundary. Remove the capability-specific parser and
lifecycle tests, and add a behavioral regression proving that an unlabeled
repo-local skill is accepted in a delivery lifecycle. Existing external skill
files that still contain `capabilities` need no compatibility code: the parser
already reads only recognized frontmatter keys, so the retired key becomes inert
like any other unknown key.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Does `task_implementation` protect delivery? | No. It is self-attested Markdown metadata. The loader verifies neither the prompt nor the resulting branch, so false labels pass and real unlabeled implementation skills fail. | Delete the semantic check instead of replacing it with another prompt classification heuristic. |
| Is skill `capabilities` used anywhere else? | Exact-reference search found one behavioral consumer: `validate_task_lifecycle_facts`. The other occurrences are parser plumbing, struct initializers, tests, docs, and the two built-in declarations. Runtime `AgentCapabilities`, typed capability Waits, and execution-boundary capability errors are separate concepts. | Remove the skill field end to end; do not touch provider, filesystem, Run-lease, or Wait capability handling. |
| Which lifecycle guarantees remain computable before launch? | `load_task_flow` proves non-empty phase-legal composition; occurrence policy proves whether a loop is human-only; the final concrete op proves settlement intent. | Retain those checks and their actionable multi-violation error. |
| Where can implementation be proved? | Only after execution, from the actual diff, tests, review, CI, PR state, and the final settlement path. The built-in `review-slice`, `gate`, and `pr land -c` path already operate at that evidence boundary. | Do not add a replacement launch-time classifier in this Task. |
| Will deleting the field require a migration or break serialized DTOs? | No durable record stores skill capabilities. `Skill.capabilities` serializes only when non-empty, and the field was added solely for this check. Frontmatter parsing manually selects known keys and ignores unknown keys. | No schema, DTO, or compatibility migration. Remove stale built-in metadata and source references. |
| Does removing the check erase `delivery` versus `design_only`? | No. `TaskOutcome` is independently persisted, shown in status, and pinned for Task identity; only its use as a capability-check switch is coupled here. | Preserve the outcome contract, but remove docs claiming that `--design-only` waives a skill-label requirement. |
| Does the default lifecycle still satisfy the contract? | Yes. `slice` contains autonomous skills and `ship` ends in `op: pr land -c`. `task-design` may retain its authored human review because the loop, not every phase, owns autonomous progress. | Default Task behavior and flow names do not change. |

The focused baseline tests currently pass and pin the unwanted behavior:
`int_10_lifecycle_names_every_missing_capability_and_correction`,
`flow_steps_cannot_declare_skill_capabilities`, and
`load_skill_parses_frontmatter_execution_contract`. They identify the exact tests
to replace rather than a broader runtime regression.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Move `task_implementation` onto a flow-level declaration | Keeps Task policy out of individual skills and makes custom flows declare their promise once. | It remains self-attested metadata with no evidence behind it, adds a second flow schema, and merely moves the false-confidence problem. |
| Infer implementation from skill or flow names | Requires no new frontmatter and can recognize built-ins. | It hard-codes Loopflow's vocabulary, rejects valid repo-local work, and directly conflicts with the zero-repo-authored-skill operating target. |
| Parse skill prose or ask a model to classify the flow before launch | Could recognize custom prompts without explicit labels. | Classification is nondeterministic, adds latency and provider dependence before every Task, and still predicts intent rather than observing delivery. |
| Remove all lifecycle validation | Makes every composition launchable. | Human-only loops and non-settling final flows are structural dead ends that Loopflow can prove before spending a provider turn. Those checks earn their place. |

## Key decisions

- Validate observable graph structure, not claimed prompt semantics.
- Remove the capability field completely rather than leaving dead parser or DTO
  plumbing.
- Treat stale third-party `capabilities` frontmatter as inert through the normal
  unknown-key behavior; add no migration, warning, alias, or compatibility path.
- Keep `TaskOutcome` as durable intent. This Task removes its invalid launch-time
  classifier, not the broader outcome API introduced alongside it.
- Accept that an autonomous design-like loop can reach the gate. The capability
  label never prevented that reliably; the final evidence-bearing review and
  settlement path is the honest enforcement boundary.

Wild success is boring authoring: an existing repo skill drops into a Project's
Task loop, launches without Loopflow-specific annotations, and is judged by what
it changed. Wild failure would be replacing the removed label with a name
allowlist or hidden built-in preference, recreating the same coupling where
custom flows are harder to see.

## Scope

- In scope: remove skill `capabilities` data/parsing/serialization, remove
  `task_implementation` declarations and lifecycle validation, replace focused
  tests, and correct authoring/Task lifecycle docs.
- In scope: preserve and prove human-only-loop and terminal-settlement refusals.
- Out of scope: provider `AgentCapabilities`, typed capability Waits, Run lease
  authority, filesystem/network/provider preflight, and their user-facing errors.
- Out of scope: removing or migrating `TaskOutcome`/`--design-only`, changing
  built-in flow composition, or adding new post-execution gates.

## Done when

- A focused Rust test creates a repo-local implementation skill with no
  frontmatter capability, selects it in a delivery Task loop, and
  `resolve_task_lifecycle` succeeds.
- The lifecycle regression still proves that a human-only loop and a finally
  flow without terminal `pr land -c` are reported together and rejected.
- `rg -n 'task_implementation|capabilities: \[task_implementation\]' rust/loopflow docs`
  returns no matches.
- `cargo test -p loopflow repo_local_task_flow_needs_no_capability_frontmatter`
  passes.
- `cargo test -p loopflow task_lifecycle_rejects_structural_dead_ends` passes.
- `cargo fmt --check` passes.
