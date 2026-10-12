# Instructions at the right level

Implementation, 2026-10-11. Jack Heart accepted the two-tier direction and
requested the change. The work contract is a skill, preloaded for associated
launches and readable on demand in an existing conversation.

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

## Placement

Unresolved; no owning Wave supplied. No planning lookup required to design this.

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

The existing `operate`/`--no-loopflow` switch still controls automatic guidance.
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

## Remaining work

One coherent local change: two-tier assembly, shared work skill, portable
onboarding/continuation/delivery instructions, external harness skill and docs.
No follow-up Tasks were filed. Lower-level worktree/PR/Flow commands
retain existing mechanics; no Task creation path was added.

The instruction split and repository ownership refinement are implemented.
Jack's 2026-10-11
repository-ownership boundary is preserved: Work context alone never authorizes `wave/`
writes; selected Flow procedures own memory maintenance. Standalone skills keep
proposed Wave changes in scratch.

Writing scratch remains available. Repository-only `scratch.managed: true`
opts into cleanup, stacked-child note isolation and persistent untracking.
Default scratch follows ordinary Git behavior. No directory-existence heuristic
or global opt-in: leftover notes cannot change ownership. The Loopflow repository
opts in explicitly to preserve its existing CI policy. Delete the unconditional
cleanup/untracking and special scratch merge/classification paths by gating them
on this one policy; preserve managed lifecycle behavior and uncommitted edits.

Acceptance adds real delivery, commit, sync and stacked-checkout coverage for
ordinary and managed scratch, plus repository-only configuration tests. No new
schema. Publication and delivery have not been requested.

## Done when

Gate runs `cargo test -p loopflow --test context_tests --test context_block_tests
--test context_launch_tests` and affected prompt/run unit tests, with inherited
`LF_*` unset and stdin closed. Tests prove taskless, Work-bound, explicit operator,
first work-API use, resume, opt-out, and provider delivery behavior. No network,
live account, display, or person is required. Run `cargo fmt --check` and
`cargo clippy --all-targets -- -D warnings` for the Rust cutover.

Base contract: 1,037 → 282 words (73% reduction).
Checks: focused Rust prompt/provenance, context reader/launch, Session CLI, command-reference and golden tests — 17 passed; `uv run pytest python/tests/test_instruction_levels.py` — 16 passed; nine decision inputs rendered and missing-response scoring failed as expected; Rust formatting/Clippy and Python Ruff passed. Live model evaluations remain for a selected provider environment.

## Instruction explanations and decision evaluations

Implemented, 2026-10-11 — Jack requested continuation after the ownership change.
The CLI exposes recorded instruction-selection reasons alongside headless decision cases
for the three usage experiences. No new user mode, store, or schema.

The prompt's optional Task/Wave scope replaces the anonymous Work boolean;
the same value selects guidance and explains it. Record inclusion, opt-out,
absence, and explicit-skill deduplication through existing `ContextDecision`s.
`lf mon show SESSION --instructions [--json]` reads those retained decisions;
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
