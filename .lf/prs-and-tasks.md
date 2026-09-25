# PR and Task authorship lessons

Recorded 2026-09-24 from the `prs-and-tasks` branch. No Wave or Project was
selected; placement remains unresolved. This is repository learning, not a
chapter plan or a claim that the branch has shipped.

## Keep the writing contract at its owners

[PROMPTS.md](../PROMPTS.md#outputs-for-humans) owns the explanation of Task,
design, PR, and report responsibilities. The independently delivered builtin
skills carry the rules each writer exercises. Avoid another full template here.

An authorship change must reach generation, gate handoff, cached copy,
publication, and mechanical rendering. Better prompts alone could not preserve
PR-specific titles while `TaskPrContext::pr_title` replaced them. Review both
the first draft and a refresh after scope changes or a second PR for one Task.

The human correction separates **Try it**, a user action and visible result,
from automated evidence in **Checks** or CI. A passing test is not evidence that
a returning reader understands the opening. Reconcile current briefs while
retaining accepted decisions and contrary evidence through durable references.

## Renderer lessons

`ops/pr.rs` preserves the authored title and first Markdown paragraph, then
inserts one managed Task block. Refresh must preserve indented Markdown and
recognize whitespace-only paragraph separators. Regression tests cover these
boundaries and repeated normalization.

`PrMergeRequest` remains the settlement authority. Publishing the same head
must describe its existing request; publication alone requests no settlement.
Read the request under the mutation lock and match its head. The landing
regression initially caught persisted CompleteTask intent paired with body text
claiming no settlement. Check persisted state and reviewer-visible copy together.
The private `TaskPrContext` read adds no stored lifecycle or DTO.

Keep authored title and body together as `PrCopy` through generation and landing.
Only CLI inputs are independently optional; resolved remote copy always has both
fields. `PrPresentation` adds the head SHA that makes persisted copy current.
Lifecycle text also represents a proposed landing disposition before the final
head exists, so rendering it must not manufacture a durable merge request.

Preserve copy resolution semantics when simplifying consumers: an explicit
title without a body leaves the body empty; a body-only override applies after
cached or generated copy is selected. Publication consumes cached copy and
deletes gate artifacts before committing; landing reads copy before clearing
scratch. Sharing the copy type must not collapse these different cleanup timings.

Landing supplies resolved copy to exact-head auto-merge. Release re-arming
supplies no copy and retains GitHub's existing text. Changes to this shared path
need landing and release consumer proof, not only parser or renderer tests.

## Carrying an existing design

CI on head `d698c3ee2f0d107b7c746ca65acd3f9b587b1c1c` exposed a stale
launch-plan assertion requiring `--first slice` after the skill moved to the
selected `--flow`. Include the builtin contract tests documented in
[TESTING.md](../TESTING.md#rust-tests) when changing builtin Markdown, even when
the focused renderer and publication tests pass.

Task creation stdin becomes the problem brief. Scratch Markdown enters prompts
from the execution worktree, including recursive untracked files; a path in a
different checkout supplies no contents. Supported guidance prepares the Task,
copies the selected design and required evidence, then runs the selected Flow.
Preserve draft status, relative references, newer destination edits, and the
existing implementation's ownership. Worker startup alone does not prove the
design reached its input.

As of this branch, `--scratch <directory>` is proposed, not implemented.
The [preserved handoff design](https://github.com/loopflowstudio/loopflow/blob/033e0758504390e5db9d254fed4964b0dd6da4bc/scratch/prs-and-tasks-handoff.md)
records source capture before remote creation, staging before launch, repeat
imports without overwriting changed files, self-contained references, and
recovery using the same Task. Checkout adoption and document availability after
scratch cleanup remain separate questions. Recheck source CLI behavior before
implementation: installed Task-start help differed from this checkout.

## Evidence and unresolved interpretation

The [slice review](https://github.com/loopflowstudio/loopflow/blob/033e0758504390e5db9d254fed4964b0dd6da4bc/scratch/prs-and-tasks-review-slice.md)
records focused renderer/publication/landing checks, static checks, and an agent
reader exercise. It does not establish human reader validation, a live design
handoff, or a full gate. These remain limits, not implied passes.

The [frozen sample](https://github.com/loopflowstudio/loopflow/blob/033e0758504390e5db9d254fed4964b0dd6da4bc/scratch/prs-and-tasks-sample.json)
and [research](https://github.com/loopflowstudio/loopflow/blob/033e0758504390e5db9d254fed4964b0dd6da4bc/scratch/prs-and-tasks-research.md)
retain original external descriptions. Completed roadmap rows with recovery
advice and queued work shown as available need semantic investigation; prose
does not establish a runtime defect. PRs #1276 and #1277 need actual diff and
dependency reconciliation before conclusions about overlap. Sampled rewrites
are proposals, not verified shipped claims or authority to change live scope.

No planning state was changed for this memory pass. Preserve scratch validation
evidence until the branch's normal delivery cleanup.
