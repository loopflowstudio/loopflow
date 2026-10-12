# Instructions at the right level

Accepted, 2026-10-11; reconciled 2026-10-12. Jack Heart accepted the two-tier
direction and requested the change. The work contract is implemented as a
skill, preloaded for associated launches and readable on demand.

## What to build

Give every lf-launched agent a small launcher contract, adding work guidance
only through use of work APIs, while keeping chosen procedures in skills.

Jack's intent:

> The wedge: Using lf as a launcher without being bought into the task management stuff

> Power users: using lf inside cmux (or rex, eventually), and using task APIs, but bringing their own UI , workflow to it

> we want to strip down the LOOPFLOW.md instructions and have a second tier that you get only when you use lf work apis not lower level APIs

Desktop supplies the guided experience. These are three ways of using Loopflow,
not three prompt presets. Desktop and custom clients share work semantics;
the caller chooses how much procedure to adopt.

## Scope

Taskless work in the existing checkout. Jack requested finishing the queue;
the calling conversation owns landing afterward. No Wave was selected.

## The demo

1. Launch `lf : "fix the failing test"` without Linear or a Task. The agent
   fixes it here; no planning lookup, Task creation, or workflow enrollment.
2. Run a taskless Skill or Flow. Its authored procedure runs without inventing
   work-management prerequisites.
3. Open the same Task through Desktop and a terminal client. Both receive its
   work contract. A plain contribution follows its request; a selected operator
   skill drives the selected procedure. Terminal choice changes neither meaning.
4. Start taskless, then read `lf help skill work` before using work APIs. The same
   guidance is available as `lf skill work`; no response-envelope or JSON change.

## Replaced system

`builtins/LOOPFLOW.md` combined tool guidance, work semantics, delivery procedure,
verification cadence, operator duties, writing style, and memory conventions.
`ProcessPromptInput.no_loopflow` becomes `GatherContextOpts.operate`, then
`format_system_sections` previously injected the whole document or none of it.
`Surface::instructions` separately distinguishes present participants from
headless execution. Preserve that independent distinction.

`build_prompt_at` already prepares explicit Task input and resolves Wave context.
Its context attribution previously assumed a single operating document.
Skills and surface prompts can reintroduce work assumptions even after that
document is shortened. The [allocation audit](instruction-audit.md) records
these seams and their allocation.

The local website's CLI copy promises no Task/Linear requirement and taskless
Flows. The homepage presents guided Workflows. Its README identifies mock
commands as intended behavior, not proof of implemented APIs. On 2026-10-11,
web retrieval returned an older public homepage and could not retrieve `/cli`;
the new website assessment uses `website/home.html` and `website/content.yaml`.

## Data structures and key functions

The internal `operate` switch controls automatic guidance. Main removed the
public `--no-loopflow` flag; standalone native skills select the omission internally.
An optional `WorkContext` (Task or Wave scope) in `ProcessPromptInput`,
`GatherContextOpts` and `PromptComponents` selects the additional contract and
explains the selection. It is derived from explicit Task/Wave scope and existing
checkout or resumed Session association. No stored
persona, new database owner or schema migration.

`format_system_sections` emits the base document and, for Work, `work_section()`.
The latter reads the body of `builtins/ops/skill/work.md`, also discovered as
`lf skill work` and `lf help skill work`. Explicitly running that skill suppresses a
second automatic copy. `attributed_context` attributes the two sections
separately. Native provider and surface instructions remain independent.

## Constraints

- Work means Task/Wave/Project planning, binding, and operations; Sessions,
  Skills, Flows, accounts, usage, machines, worktrees, and PRs remain usable below
  that layer. Work-scoped forms of these APIs can select work guidance.
- Explicit Work scope, an existing Task checkout, and resumed Work Sessions
  receive tier two. Other Tasks in the repository do not enroll this checkout.
  This adopts the narrow existing-association interpretation of Jack's detection
  suggestion; association still selects no procedure.
- Work guidance describes identity, state, and how to operate APIs. Continuous
  supervision, workflow selection, and completion procedures require their
  selected skill or an explicit request, including in Desktop.
- Repository instructions remain authoritative for repository conventions.
- No schema migration expected. Resume/replacement and mid-conversation entry
  must work without a new parallel instruction-state store.

## Delete — do not maintain

The cuts below are complete. `ScratchConfig`, `managed_scratch`,
`UNTRACK_SCRATCH`, `prepare_persistent_publication`,
`untrack_persistent_scratch`, and `restore_scratch_from_head` are absent.
Their replacement is the shared diff selector and explicit cleanup operation;
no predecessor path remains to maintain.

- The all-or-nothing full operating document and its full-document assumptions
  in `builtins.rs`, `prompt/mod.rs`, and `commands/run.rs` attribution. Preserve
  opt-out, source attribution, native instructions, and exact-once coverage.
- Unconditional Work doctrine in surface prompts and generally useful skills.
  Move each rule to the work document or the skill that exercises it; do not
  preserve two copies or fork every skill into launcher/Desktop variants.
- Mandatory managed onboarding in `init`; implicit promotion in `default` and
  `advance`. Preserve explicitly requested setup and Task operations.
- Tests that require universal Work prose. Replace with behavioral coverage
  proving the two selections and their absence conditions.

## Forbidden outcomes

Host detection choosing policy; a Task automatically selecting an operator;
taskless Flow execution requiring Linear; a tiny base prompt whose skills still
force Task creation; work guidance existing only at fresh launch; JSON polluted
by prose; hiding command semantics behind a user-level setting.

## Implemented behavior

One coherent local change: two-tier assembly, shared work skill, portable
onboarding/continuation/delivery instructions, external harness skill and docs.
No follow-up Tasks were filed. Lower-level worktree/PR/Flow commands
retain existing mechanics; no Task creation path was added.

The instruction split and repository ownership refinement are implemented.
Jack's 2026-10-11
repository-ownership boundary is preserved: Work context alone never authorizes `wave/`
writes; selected Flow procedures own memory maintenance. Standalone skills keep
proposed Wave changes in scratch.

Jack revised the ownership mechanism on 2026-10-12: use the Git diff, with no
scratch configuration or reserved directory. Skills keep their existing paths.
Cleanup removes added scratch files; files present at the base and edits to them
survive. Detected renames and `.gitkeep` are preserved. All checkouts use
ordinary Git behavior for scratch. `lf pr submit` and `lf pr land` invoke
`lf scratch clear` during preparation; it is also available independently.
Default Flows need no extra step. PR open and publish do not run general scratch
cleanup. Scratch writing and automatic context inclusion remain unchanged.
Stacked checkouts and merges use normal Git behavior for inherited content.

One diff-based selector replaces the deleted scratch configuration and cleanup
paths. Cleanup compares the base with committed HEAD; untracked and staged-only
additions are outside that selection. Delivery checkpoints intended work before
cleanup. Existing-file index and working edits survive. No ownership manifest,
schema, enrollment or additional setup was added.

Onboarding infers the requested capability instead of requiring a three-way
choice. Automatic Work association, skill loading and direct skill execution
remain. Scratch curation may edit existing documents; delivery removes only
added paths. Work association remains context, not a selected procedure.

Compression removes repeated opt-out and sync classification branches and
redundant stacked-checkout fixture setup. Review corrected a comment that
conflated product cleanup with hosted CI: this repository still requires an
empty scratch tree except `.gitkeep`; its CI workflow and policy tests are unchanged.

Compression was checkpointed in `90c8692fa3`; no interrupted edits remained at
reconciliation. Main through `9d0d41f5e0` is integrated. Its unified history
command now owns `lf history show SESSION --instructions`; `297901cee3` carries
the feature and documentation there. Main also removed public `--no-loopflow`:
`5341439ffe` updates omission explanations to describe the internal automatic
guidance switch without naming a removed flag. Standalone native skills can
omit automatic guidance; selected skills retain their own provenance.

## Done when

Gate runs `cargo test -p loopflow --test context_tests --test context_block_tests
--test context_launch_tests` and affected prompt/run unit tests, with inherited
`LF_*` unset and stdin closed. Tests prove taskless, Work-bound, explicit operator,
first work-API use, resume, opt-out, and provider delivery behavior. No network,
live account, display, or person is required. Run `cargo fmt --check` and
`cargo clippy --all-targets -- -D warnings` for the Rust cutover.

Base contract: 1,037 → 282 words (73% reduction).
Earlier compression evidence at `90c8692fa3`: task initialization and sync tests
passed (41), additive guidance passed (1), CI-policy tests passed (35), and fmt,
Clippy and diff checks passed. Post-sync evidence records context launch (8),
instruction inspection (1) and additive guidance (1) passing. These focused
results do not establish the full post-sync gate or a live model evaluation.

## Instruction explanations and decision evaluations

Implemented, 2026-10-11 — Jack requested continuation after the ownership change.
The CLI exposes recorded instruction-selection reasons alongside headless decision cases
for the three usage experiences. No new user mode, store, or schema.

The prompt's optional Task/Wave scope replaces the anonymous Work boolean;
the same value selects guidance and explains it. Record inclusion, opt-out,
absence, and explicit-skill deduplication through existing `ContextDecision`s.
`lf history show SESSION --instructions [--json]` reads those retained decisions;
old captures report only what they recorded, never inferred historical authority.
Keep `--context` token accounting intact. Selected skills, repository guides,
provider instructions and surface guidance retain separate provenance.

Decision evaluations use production-rendered prompt fixtures and score an
agent's proposed actions against allowed and required actions. They cover plain
launcher work, a taskless Flow, custom Work use, guided memory maintenance,
explicit adoption, opt-out and repository-owned scratch. These are decision
evaluations, not proof of actual tool effects. Deterministic tests validate
selection and scoring; live provider runs remain a separate headless check.
Acceptance: focused prompt/provenance and CLI tests, evaluation runner tests,
formatting and Clippy. Delete the unexplained Work boolean and opaque automatic
instruction reasons; reuse existing captured decisions and rendering machinery.

## Remaining work and evidence limits

- The queue's gate still owns affected-suite verification against the integrated
  implementation, including the acceptance commands above. Existing results are
  reusable only where their content and command plan remain applicable.
- Hosted CI retains its empty-scratch policy. All three current scratch notes
  are additions relative to main, so delivery cleanup can remove them without
  weakening that policy or deleting pre-existing repository content.
- Nine provider-neutral decision cases and a scorer exist. Live provider runs
  remain unperformed; model/version and evaluation environment are unselected.
  This is the outstanding evaluation choice, not evidence of failed behavior.
- No bounded implementation repair was identified during reconciliation. The
  instruction split, capture explanations and diff-based cleanup remain the
  accepted implementation; publication and landing remain with the caller.

Check (2026-10-12): `git diff --check` passed; `lf context` reports scratch within both targets, with one existing audit-table wording warning; no code changed and gate owns remaining verification.
