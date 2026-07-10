# Research: Linear as Loopflow's project/task store

## System understanding

The current adapter maps one Loopflow wave to one Linear Project. Loopflow's
actual projects remain Markdown files under `wave/<wave>/projects/`; tasks are
Linear Issues inside the wave-level Linear Project and use `project:<slug>`
issue labels for project membership.

This collapses two domain levels into one provider object:

```text
Loopflow wave     -> Linear Project
Loopflow project  -> Linear issue label + local Markdown file
Loopflow task     -> Linear Issue
```

Linear's native planning hierarchy is a closer match:

```text
Loopflow wave     -> Linear Initiative
Loopflow project  -> Linear Project
Loopflow task     -> Linear Issue
```

Linear describes Initiatives as goal-oriented groups of Projects, Projects as
units of work comprised of Issues, and Issues as belonging to at most one
Project. Initiatives, Projects, and Issues are available across Linear plans.

Sources:

- https://linear.app/docs/initiatives
- https://linear.app/docs/projects
- https://linear.app/pricing
- https://github.com/linear/linear/blob/master/packages/sdk/src/schema.graphql

### Architecture

`PmContext.project` currently holds the wave's Linear Project ID. `pm init`
creates that Project and writes `pm.linear_project` to `GOAL.md`. Task creation
passes that ID as `IssueCreateInput.projectId`; the local project is separately
ensured as an issue label. `pm show`, `pm status`, and `pm sync` reconstruct
project membership by parsing those labels.

The provider DTOs reflect the flattened model. `PmProject` carries only `id`
and `name`; `PmItem` carries labels but no native parent Project. The Linear
adapter already has Project and Issue CRUD, but no Initiative operations and no
Project content/summary reads.

### Linear API shape

The current GraphQL schema exposes the required native relations and mutations:

```rust
struct Initiative {
    id: String,
    name: String,
    description: Option<String>, // short summary
    content: Option<String>,     // Markdown
    projects: Vec<Project>,
}

struct Project {
    id: String,
    name: String,
    description: Option<String>, // short summary
    content: Option<String>,     // Markdown
    initiatives: Vec<Initiative>,
    issues: Vec<Issue>,
}

struct Issue {
    id: String,
    description: Option<String>, // Markdown
    project: Option<Project>,
}
```

`initiativeCreate`, `initiativeUpdate`, `initiativeToProjectCreate`,
`projectCreate`, `projectUpdate`, `issueCreate`, and `issueUpdate` are public
mutations. A Project can belong to multiple Initiatives; an Issue can belong to
only one Project. Loopflow can enforce exactly one *Loopflow wave* association
while leaving unrelated Linear Initiative associations alone.

### Human surface

Linear Project Overview has both a brief summary and a detailed description.
The detailed description uses the same Markdown editor as documents, is shown
on the Project Overview, supports inline comments, and has history. Project
documents are a valid alternative, but add another object and another
navigation step.

Source: https://linear.app/docs/project-overview

## Tensions

- **Provider hierarchy versus Loopflow hierarchy**: the current provider type
  named `PmProject` represents a wave container, while the Loopflow project is
  only a label. The same noun means two different things at the boundary.
- **Authority versus availability**: making Linear authoritative removes the
  Git/Linear split, but wave loops then need live or cached Linear project specs
  at every pass boundary.
- **Typed KRs versus Markdown storage**: Linear has no native KR entity. KRs
  need a stable Markdown convention and a typed Loopflow projection.
- **Many-to-many Initiatives**: Linear permits a Project in multiple
  Initiatives, while Loopflow requires every Project to belong to exactly one
  wave. Provider flexibility must not weaken the Loopflow invariant.

## Observations

### Complexity

Most PM reconciliation complexity exists only because project membership is
encoded twice: local project filenames establish the valid set, while Linear
labels attach Issues to that set. `pm sync` must reconcile files, labels,
issues, and the wave-level Linear Project.

### Quality

Task title, description, completion, and native Project attachment already map
cleanly to Linear. Project summaries and detailed content are currently left
blank or unread, even though the GraphQL API and UI support both.

### Potential

Linear Project content is the best native home for project definition and KRs:

```markdown
## Definition

<what this measured bet means>

## KRs

- [ ] <observable proof condition>
- [ ] <observable proof condition>
```

The short `description` field should carry a one-sentence summary suitable for
Project lists. `content` should carry the detailed definition and KRs. Project
updates should report progress against KRs, not define them. Milestones should
remain phases/checkpoints, and Issues should remain concrete work.

Loopflow should parse the Markdown into a domain type rather than expose the
storage convention everywhere:

```rust
struct PmWave {
    id: String,
    name: String,
    summary: String,
    projects: Vec<PmProject>,
}

struct PmProject {
    id: String,
    slug: String,
    name: String,
    summary: String,
    definition: String,
    krs: Vec<PmKr>,
    tasks: Vec<PmTask>,
}

struct PmKr {
    text: String,
    holds: bool,
}
```

## Open questions

- Should checking a KR be a human/project-loop judgment stored as `[x]`, or
  should `holds` be derived from linked evidence?
- Should `wave/<wave>/projects/*.md` disappear, or remain a read-only generated
  cache for offline prompt assembly? An editable two-way mirror would recreate
  the source-of-truth problem.
- Should a Linear Project associated with two Loopflow-managed Initiatives be a
  hard `pm doctor` error? It likely should.
- How should standing quality-frontier projects map to Linear's bias toward
  projects with clear completion dates? The API permits projects without a
  target date, so this is a product convention rather than a schema blocker.

## Recommendations

### Use Linear's native hierarchy

**Observation**: Initiative -> Project -> Issue directly matches Wave ->
Project -> Task, and all required reads/writes exist in the public API.

**Cost**: Replace wave-level `linear_project` with `linear_initiative`, migrate
existing labels into Linear Projects, and reshape PM DTOs/commands around the
hierarchy.

**Benefit**: Project membership becomes native and visible in Linear; the PM API
stops leaking provider labels as domain structure.

**Verdict**: Worth it. This removes the largest conceptual mismatch in the PM
layer.

### Store definition and KRs in Linear Project content

**Observation**: Project content is Markdown, visible on the Project Overview,
and round-trips through GraphQL. No other Linear object matches durable
completion criteria as well.

**Cost**: Define and validate a small Markdown schema, add typed parsing, and
choose a cache/offline policy.

**Benefit**: Humans and agents read the same project definition and KRs in the
same system that owns its tasks. Summary fields make project lists useful.

**Verdict**: Best available home. Keep KRs as a Loopflow concept with a Linear
Markdown representation; do not turn them into milestones or issues.

### Split authority by domain, not by field

**Observation**: Wave runtime configuration, cadence, and memory do not fit
Linear, while project planning and tasks do.

**Cost**: Context assembly must fetch or cache Linear's project tree.

**Benefit**: A clean authority boundary: the repo owns wave operation; Linear
owns project inventory, definitions, KRs, status, and tasks.

**Verdict**: Prefer this over keeping editable project specs in both places.
