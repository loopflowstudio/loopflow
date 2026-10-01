# A responsive default conversation

Implementation authorized by Jack Heart — 2026-10-01. Jack Heart requested: “make lf less of an opinonated default” because “the Desktop app + repo/wave agents cover a lot of the lf operational ground.” Earlier direction: “Prioritize responsiveness” and a “possible request is unrelated.”

## What to build

Bare `lf` opens a general-purpose conversation that follows the participant's direction and brings in Loopflow coordination when useful.

Jack Heart proposed merging the operational skills under `lf operate` on
2026-10-01. This supersedes the earlier proposal to retain `lf loopflow`.
The existing review skill is named `review-open-work` in the repository.
Jack Heart then selected `repo/operate` as the canonical skill, with `operate`
as its short name. One explicit resolver shorthand handles the otherwise
ambiguous suffix shared with `wave/operate`; no duplicate skill is registered.
The separately published `skills/loopflow/SKILL.md` teaches external harnesses
the API and remains unchanged.

## Placement

Unresolved; no Wave was supplied. This bounded default-launch change needs no planning-provider lookup to design.

## Implemented behavior

`run_default_agent` in `rust/loopflow/src/bin/lf.rs` selects `default` in
both existing worktree-placement branches. The prompt lives in
`rust/loopflow/src/engine/builtins/ops/skill/default.md`: follow the conversation,
answer unrelated requests directly, work here, and load `repo/operate` when
coordination helps within existing authorization. With no request, invite one.

Prompt preparation and execution in `rust/loopflow/src/lf/commands/run.rs`
share `forced_launch_target`. The default stays in the terminal, including
when an IDE mode is supplied. Explicit operation honors normal launch settings.
The existing registry and prompt assembly suffice; no new domain values,
configuration, or persistence were needed.

## Explicit operation

`ops/skill/repo_operate.md` consolidates `loopflow` and
`review-open-work`. Its purpose: inspect current work, identify what needs
attention, and carry authorized actions through to an observable result.

Jack Heart clarified the operational agenda on 2026-10-01: clear obsolete
worktrees, branches, PRs, Tasks, and Loopflow database state; summarize activity
and provide updates; apply `wave/operate` per Wave as needed to advance Tasks;
and be ready to capture new Tasks. This supersedes the request-first framing.
Task capture does not imply immediate execution. Cleanup uses the state owner's
supported operations and existing authorization; missing database cleanup
capabilities become concrete Tasks rather than direct SQLite edits.

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

## Reconciliation — 2026-10-01

The default launch, skill consolidation, discovery shorthand, and documentation
are implemented. The retired builtin skills, launch special cases, and old-name
references in active docs/tests are removed. The external harness skill remains.
One PR remains the accepted delivery unit; no follow-up Tasks are identified.

Review finding: prompt assembly and execution previously disagreed on the
forced terminal surface when an IDE mode was supplied. They now share one
launch-target decision. Tests cover default selection, terminal routing,
procedure ownership, and canonical overrides for the `operate` shorthand.

The operational skill keeps Task-description and Flow-selection guidance in
one place. Installed Flows own their step order; the skill no longer copies
that order or directs routine progress into Task comments.

Merged upstream change 16f03bb37 reduces scratch context to 12,000 tokens / 96 KiB.
Its context-ablation findings and `wave/intelligence/MEMORY.md` support keeping
working notes focused; they do not establish conversational quality. Duplicate,
stale prompt snapshots are replaced with links to the current source. This work
has no identified Wave, so no Wave memory is assigned or changed.

## Remaining work

Full affected suites and clippy remain for gate. Conversational responsiveness
and operational triage still need demo/review judgment; stub-provider checks
prove prompt delivery, not model behavior. No product decision is pending.

## The demo

Open `lf`: a short invitation, no operational inventory. Ask an unrelated
question: receive a direct answer. Ask for a small local edit: work proceeds
here. Describe a substantial effort: coordination becomes available in
response to its needs. `lf operate` offers open-work triage; `lf operate` with
a specific request goes directly to the relevant work.

## Done when

Headless gate: `cargo test -p loopflow --test default_conversation_tests` verifies
the default prompt delivered to a stub provider. `cargo test -p loopflow --lib
bare_lf` verifies terminal routing.
`cargo test -p loopflow assembled_prompts_deliver_procedures_to_the_owning_skill`
passes with operational procedures confined to their owning skills.
Existing discovery coverage proves `operate` resolves to `repo/operate`, including
canonical overrides, while `wave/operate` remains distinct; run `cargo test -p loopflow lf::discovery`.
`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass.
Conversational responsiveness is judged in demo/review, not by brittle prose
assertions or a live-provider gate.

Check: `git diff --check` passed (realign, prose only); prior focused prompt/default/discovery checks and formatting passes retained as earlier evidence, not rerun after upstream integration; affected suites and clippy deferred to gate.
