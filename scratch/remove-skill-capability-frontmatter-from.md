# Make Task Lifecycles Structural and Preserve Task PR Identity

## Problem

Task lifecycle validation currently predicts delivery from
`capabilities: [task_implementation]` on a skill in the first or loop flow.
That declaration is self-attested Markdown metadata: an implementation prompt
without the label is rejected, while a non-implementing prompt with the label
passes. The accompanying `delivery` / `design_only` Task outcome exists only to
switch that semantic check on or off, so the flag, durable state, and DTO surface
encode a distinction that the runtime cannot prove.

The default feature lifecycle also stops at the wrong evidence boundary. It
asks the human to approve the design before implementation, but `ship` can then
settle without letting the human review the real configured-path behavior.
Fix work has the opposite and intentional contract: restore first, then ask for
human judgment only when the working demo exists.

Finally, Task PR copy is persisted per head but is not bound to Task identity.
Generated, cached, or explicitly supplied copy can replace the Linear Task name
and omit its link. That makes the PR harder to trace and lets later refreshes or
serial PRs erase the association.

These are one Task API problem: launch should validate what the expanded graph
can prove, human gates should sit at the two evidence boundaries that matter,
and every published result should retain the identity of the Task that owns it.

## The demo

Run a feature Task with a repo-local implementation skill that has no capability
frontmatter. The Task launches, parks once for design review, implements through
the custom loop, then parks again on `demo` with the changed behavior exercised
through the repository's real configured path. Accepting that demo is the last
human action before `pr land -c` settles the Task.

Publish the Task PR with deliberately generic authored copy. GitHub still shows
the current Linear Task name at the start of the title and a direct Linear Task
link in the body, while retaining the useful authored context. Refresh the PR
and rotate to a later serial PR; both retain the same identity anchors.

## Confirmed product contract

### One Task model

Delete `TaskOutcome` and its `Delivery` / `DesignOnly` variants. Delete
`--design-only`, `TaskFlowOverrides.outcome`, lifecycle outcome persistence,
status/JSON output, waiver logic, documentation, and focused tests. Restore
`TaskLifecyclePlan` to the three pinned phase flows only:

```rust
pub struct TaskLifecyclePlan {
    pub first: TaskPhasePlan,
    pub loop_: TaskPhasePlan,
    pub finally: TaskPhasePlan,
}

impl TaskLifecyclePlan {
    pub fn standard(
        first_flow: impl Into<String>,
        loop_flow: impl Into<String>,
        finally_flow: impl Into<String>,
    ) -> Self;
}
```

A Task may deliver code, investigation, documentation, or a design artifact.
Its Linear directive and selected flows express that work; there is no second
durable outcome label and no compatibility alias.

The outcome column was introduced in the still-draft
`task_lifecycle_contract` migration. Remove it from that draft and from all
explicit Task SQL columns and parameters. Keep the PR presentation columns from
the same draft. Do not add a compensating migration or retain reads for local
databases that happened to run the earlier draft; an orphaned draft column is
ignored by the explicit column lists.

### Structural lifecycle validation only

Remove `Skill.capabilities`, `SkillFrontmatter.capabilities`, parsing and
serialization, the special flow-step rejection, both built-in
`task_implementation` declarations, and every implementation-presence branch
from `validate_task_lifecycle_facts`.

Retain only facts computable from the expanded flow:

- each phase resolves to a non-empty flow with phase-legal step kinds;
- first and loop contain skills only;
- finally contains one or more skills followed by optional ops;
- the loop contains an autonomous skill occurrence rather than only human
  nodes;
- the final concrete step is `op: pr land -c` or `op: pr land --complete`.

Describe refusals in structural language. A human-only loop "has no autonomous
skill step"; a non-settling final flow "does not end with `op: pr land -c`."
Do not describe either property as a declared capability.

Unknown `capabilities` keys in existing skill frontmatter become inert through
the parser's ordinary unknown-key behavior. Add no warning, migration, alias,
name allowlist, prose classifier, or flow-level replacement declaration.

### Two gates for default feature work

Change both the default Task lifecycle and the `--feature` / `--feat` preset to:

```text
first:   task-design   # kickoff -> human review-design (review_kickoff)
loop:    slice         # autonomous implementation and review
finally: ship-demo     # task-gate -> human demo (review_demo) -> land -c
```

The two gates are distinct and ordered:

1. `review_kickoff` lets the human reshape the design before implementation.
2. `review_demo` lets the human review real configured-path behavior after
   implementation and before Task settlement.

Keep the fix preset exactly at its current boundary:

```text
first:   incident
loop:    slice
finally: ship-demo
```

Fix work has one human gate, `review_demo`. It does not acquire a design-review
gate.

`TaskCycle` remains a launch-time preset, not persisted Task identity. Project
flow configuration and explicit `--first`, `--loop`, and `--finally` overrides
remain composable and may choose other lifecycles when they pass the structural
checks above. Selecting `--feature` guarantees the two built-in gates unless an
explicit phase override deliberately replaces part of that preset; no hidden
"feature Task" state survives launch.

### Task PR identity anchors

Every GitHub PR owned by a Task carries identity derived from the current
durable Task and cached PM snapshot:

- title anchor: the exact current `task.plan.title` at the start of the GitHub
  title;
- body anchor: `Linear Task: [<identifier>](<provider issue URL>)`;
- URL source: the `PmItem.url` matched by stable Linear issue UUID in the owning
  Wave's cached PM snapshot.

Do not copy the provider URL into `TaskPlan` or add another persistence field.
The PM snapshot already owns that fact. Publication is cache-only and performs
no surprise Linear refresh or network read. If the owning snapshot, issue, or
valid HTTP(S) issue URL is absent, refuse Task PR publication before push or any
GitHub mutation with an actionable `lf pm sync --wave <wave>` correction.

Normalize raw copy after generated, cached, and explicit copy converge, but
before either durable publication intent or GitHub create/update:

```text
<Task name>
<Task name> — <authored title>   # when authored title adds distinct context

Linear Task: [LOO-230](https://linear.app/...)

<authored body>
```

If the authored title already begins with the exact current Task name, preserve
it rather than duplicating the anchor. If the body already contains the exact
current canonical link line, preserve one copy. When a title-length boundary
forces a choice, remove or truncate only the optional authored suffix; never
truncate the Task-name anchor. If the Task name alone cannot be published,
refuse before side effects.

Make the normalized copy the single value passed to both Task publication
persistence and GitHub. Revalidate the anchors at the durable request boundary
so no alternate caller can persist unanchored Task copy. Non-Task PRs pass
through unchanged.

Both existing publication routes must use this contract:

- `lf pr publish` / `lf pr open` through `create_or_update_pr`;
- `lf pr submit` / `lf pr land` through the land finalization path.

Because both routes operate on the active `TaskPr`, the same normalization
applies to refreshed heads and every successor created by serial PR rotation.
The stored `PrPresentation` remains head-pinned and contains exactly the copy
sent to GitHub.

## Integration shape

Keep policy in the Task PR boundary rather than teaching `pr-message`, `gate`,
or individual agents about identity. Those producers continue to author useful
review context; the Task-owned publication path adds and validates the durable
anchors.

The implementation should have three small responsibilities even if exact
names differ:

```rust
struct TaskPrIdentity {
    title: String,
    identifier: String,
    issue_url: String,
}

fn load_task_pr_identity(repo: &Path) -> OpsResult<Option<TaskPrIdentity>>;
fn anchor_task_pr_copy(identity: Option<&TaskPrIdentity>, copy: PrCopy)
    -> OpsResult<PrCopy>;
fn request_task_pr_publication(repo: &Path, copy: &PrCopy) -> OpsResult<bool>;
```

Load and validate Task identity before a publication path crosses its first
remote side-effect boundary. Resolve the raw copy through the existing explicit
/ cached / generated precedence, anchor it once, then use that same copy for
the stored request and GitHub command. Preserve the existing head and branch
fences around publication.

## Behavioral proof

### Lifecycle

- A repo-local skill with plain Markdown and no capability annotation can serve
  as the loop of a structurally valid Task lifecycle.
- A human-only loop and a finally flow without terminal `pr land -c` are both
  reported in one actionable refusal.
- Default and `--feature` expansion contain exactly two human occurrences in
  order: `review_kickoff`, then `review_demo`.
- `--fix` expansion contains exactly one human occurrence, `review_demo`, and
  no `review_kickoff`.
- Clap rejects the deleted `--design-only` flag.
- Task JSON and persistence round trips contain only the three lifecycle flows.

### PR copy

- A Task publication given a generic title and body sends anchored copy to the
  intercepted GitHub create/update command and persists that exact anchored
  `PrPresentation` for the current head.
- A refresh with replacement authored copy retains one current title anchor and
  one current Linear link while preserving the replacement context.
- The same anchoring helper is exercised against a sequence-2 active Task PR,
  proving serial PRs do not bypass it.
- An absent/null/invalid cached issue URL refuses before Git push or a GitHub
  command and records no publication request.
- A non-Task PR keeps its authored title and body byte-for-byte.

Prefer extending the existing Task PR integration fixtures and intercepted
`gh` scripts over tests of helper wiring. Assert the copy visible at the product
boundary and in durable state.

## Documentation

- Remove capability declarations and guidance from skill authoring docs.
- Remove `--design-only`, outcome, and waiver examples from Task docs and CLI
  help.
- Describe Task validation as structural composition plus evidence-bearing
  review, CI, demo, and settlement.
- Add `ship-demo` to the built-in flow reference and describe the default /
  feature two-gate sequence and fix's single demo gate.
- Document the Task-name and Linear-link PR invariant where publication and
  serial PR behavior are explained.

Do not touch provider `AgentCapabilities`, execution-boundary preflight,
filesystem/network permissions, typed capability Waits, or Run lease authority.
Those are runtime capabilities with observed failure modes, not skill claims.

## Alternatives rejected

| Approach | Why it loses |
|----------|--------------|
| Move `task_implementation` to flow frontmatter | Moves the same unproved claim into a second schema. |
| Infer implementation from skill/flow names or prose | Rejects legitimate custom work and predicts semantics rather than observing results. |
| Keep `design_only` as descriptive metadata | Leaves an otherwise behaviorless parallel Task type in storage, DTOs, and CLI. The directive and lifecycle already express the work. |
| Require both human gates for every custom lifecycle | Breaks the intentional fix contract and turns launch presets into hidden persisted kinds. Structural custom composition remains valuable. |
| Teach every PR-copy author to include Task identity | Generated, cached, explicit, refreshed, and serial paths can drift. The owning publication boundary is the only complete enforcement point. |
| Persist the Linear URL again on `TaskPlan` | Duplicates a provider fact already keyed by stable issue UUID in the PM snapshot. |
| Reject otherwise useful PR copy that omits anchors | Makes authors memorize machine-owned boilerplate. Deterministic normalization preserves their context and the invariant. |

## Scope

In scope:

- remove skill capability schema and semantic lifecycle validation;
- remove the `design_only` Task distinction end to end;
- change default / feature gates while preserving fix gates;
- enforce Task PR title/link identity across all publication paths;
- update focused behavioral tests, CLI help, and user docs.

Out of scope:

- runtime provider/filesystem/network capabilities and preflight;
- changing Task phase repetition, gate resolution, CI, merge, or settlement
  authority;
- persisting a feature/fix Task kind;
- requiring the built-in human gates in arbitrary custom lifecycles;
- fetching Linear during PR publication;
- changing non-Task PR copy.

## Done when

- `rg -n 'task_implementation|capabilities: \[task_implementation\]|design_only|design-only' rust/loopflow docs`
  returns no Task-lifecycle matches.
- Focused lifecycle tests prove unlabeled custom work, the two default/feature
  gates, the single fix demo gate, and both retained structural refusals.
- Focused PR tests prove initial, refreshed, and sequence-2 Task PRs retain the
  current Task-name/title and Linear-link/body anchors through the real
  publication boundary.
- Missing Linear URL evidence fails before remote mutation; non-Task PR behavior
  is unchanged.
- `cargo fmt --check` passes.
- `cargo clippy --all-targets -- -D warnings` passes for the affected crate.

## Human review

Confirmed in the `review_kickoff` session on 2026-08-20:

- one Task model with no design-only outcome;
- structural lifecycle validation with no skill semantic claims;
- two gates for default/feature work and only the demo gate for fixes;
- custom flow overrides remain structurally composable;
- canonical Task-name and Linear-link PR anchors are inserted centrally while
  preserving authored reviewer context.
