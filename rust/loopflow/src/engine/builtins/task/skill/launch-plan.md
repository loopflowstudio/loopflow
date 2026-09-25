---
requires: scratch/<branch>.md
produces: one core for this Task | live follow-up Tasks
action_style: procedural
---
Turn the design into an execution decision using the Task controls that already
exist. Do not create a manifest, receipt, marker, or new planning state.

## Orientation

Read the design, Task directive, current Work state, and repo guide. Consult
Wave chapter state only when the seed identifies it.

## Decide what stays here

Choose the ambitious single-threaded core whose implementation will settle the
contract for the rest of the work. Keep that core in this Task and describe its
boundary clearly enough for the following `implement` step. Avoid scaffolding:
the core should ship useful end-to-end behavior in this PR.

## Decide what becomes Tasks

For each remaining independently shippable outcome, choose one of two actions:

- If it can safely start against the current contract, create and launch it now
  when no local artifact needs staging:
  ```bash
  lf task start --wave <wave> "<desired experience>" --flow <chosen-flow> <<'BRIEF'
  <short user-problem brief; durable design reference>
  BRIEF
  ```
  Stdin files the Task description; `--directive` supplies worker direction,
  not that durable brief. Use `--stack-on <current-task>` when it must build
  on this Task's branch.
- If it depends on decisions this core has not settled, leave it in the design
  as a named follow-up. Create it after this PR settles; do not invent durable
  staging state inside Loopflow.

Write the Task from the user's perspective. Explain what they're trying to do, what
gets in their way, and why it matters. Give it a title naming the problem or desired
experience. Show what success would look like in a concrete moment of their work.
Preserve the user's own language when it anchors intent.

Ground the Task in observations, real constraints, and examples of success. Possible
solutions can help explain the idea; mark what remains uncertain. The design doc develops
the architecture, APIs, implementation sequence, and verification. A Task is ready for
design when the problem is clear, even if the solution isn't. Link accepted decisions
and keep them binding.

Keep the Task useful to someone choosing what to work on now. As understanding changes,
update the problem and desired experience. Keep blockers and decisions that affect that
choice visible, with links to evidence.

Preserve earlier reasoning in durable records without making readers replay every
checkpoint. Retain unresolved constraints, contrary evidence, and recorded decisions.
Give follow-up Tasks independently useful outcomes, rather than implementation layers.

Link related work where you explain its relevance. In prose and PR bodies, use
`[Title · Task ID or PR number](known URL)` on first mention; shorten later references
when unambiguous. State the relationship, such as builds on, supersedes, or verified by.
Use known URLs and preserve cited decisions and evidence somewhere that survives shipping.
In operational lists, put the ID first: `[Task ID or PR number · Title](known URL)`.

For already designed work, select a Flow that continues that design. The
user's explicit Flow selection governs; otherwise use the Wave chapter recommendation.
Keep the design and its evidence available in each execution context before
launch; use staged preparation below when artifacts must cross contexts.

Finish with a short accounting of the core retained here, Tasks launched, and
follow-ups intentionally deferred.

## Existing-design handoff

When the user asks to file a Task and run a Flow from an existing design,
keep the design separate. Reuse the Task if it is the same work; otherwise file
a short user-problem brief under the selected Wave, with a design reference,
its maturity, and open questions. Do not invent ownership.

```bash
lf pm task create --wave <wave> --title "<desired experience>" --notes "<brief; design reference and maturity>"
lf task prepare <issue> --json
# Copy the selected design and required evidence into the returned worktree's scratch/.
lf task run <issue> --flow <chosen-flow>
```

Inspect the current context first: a design already in the Task worktree needs
no transfer. For a separate source, copy the actual documents and supporting
files before launch, preserve relative references, and check their contents in
the destination. A path alone does not supply context. Preparation launches no
worker; put any initial directive on preparation, since an already prepared
Task rejects a new `run --directive`. Do not overwrite newer destination work.

The destination becomes the working design; retain source provenance without
maintaining competing active copies. Markdown under its recursive `scratch/`
tree enters worker context; other assets remain on disk. Preserve material
needed after scratch cleanup in durable documentation or existing records.
Do not pipe the design into Task creation: stdin becomes the Task description.

Use the Flow the user selected and inspect its contents when explaining where it
begins; otherwise use the Wave chapter recommendation. Continue the design already
present without treating its draft choices as approved. Report the Task link,
destination design path, selected Flow, and observed launch result. Verify
supplied context separately from worker startup.

If implementation already exists in the source checkout, preserve it and its
writer. Document transfer does not adopt a checkout; current preparation does
not adopt an unbound existing branch/worktree. Report that gap before launching
a competing implementation. No automatic scratch-transfer flag is available.
