# DTO wire fixtures

Each fixture pins one live wire shape. Swift fixtures cover the per-Wave
listener and `lf wave status` contracts consumed by the Mac app. Every absent field
is a parse error or an explicit null.

`task_status.json` pins the planning/execution envelope for available, unavailable,
invalid, removed and absent planning. Rust round-trips it. This CLI-only contract
has no Swift mirror: the Mac app reads a Task's work from the workspace stream's
`task` part (`work_frame.json`). These cases have no execution.

`task_execution.json` pins the execution boundary inside the status envelope's
optional Task snapshot (`execution.execution`).
This CLI-only contract is tested in Rust. The Mac app receives its derived
condition, reason, and actions through the existing Wave Task row.

`session.json` pins `lf session connect --json`: one unresolved Task FlowStep
session, its explicit readiness state, and its exact Machine-local attach route.

`session_memberships.json` pins each Session's required `flow_membership`:
the current Flow step, an earlier step, an independent conversation, a Session that
predates recorded membership (unknown, never independent), and a remote Flow
Session whose canonical name lives on its Machine (`title_source: unavailable`).

`task_condition_states.json` pins the Rust-owned Task condition fold for clean
backlog, completion, external and review waits, local recovery blockers, and
unavailable evidence. Rust and Swift decode the same Task rows; consumers never
reconstruct the condition from process flags.

`activity_snapshot.json` pins `lf ps --json`: exact live Process and provider
processes carry OS-derived state, while a provider without exact ownership
stays separate from the call tree. Desktop receives the same wire type through
the Work observation stream; it does not poll `lf ps`.

`session_history_summary.json` and `wave_detail.json` pin the `SessionHistory`
shape used by `lf usage --json` and Wave detail. Captured event sequences
and native thread/turn references retain distinct outcomes. Optional counters,
missing capture/start membership, stream finality and evidence gaps stay explicit.
The optional `task_pr_id` retains the PR captured by the managed Flow;
`first_provider_attempt_at` differs from capture/import observation time.

`work_activity_snapshot.json` pins `lf history --json`: durable Work creation,
Session capture/provider history, PR and Steer facts retain their original
identities. Rust and Swift round-trip this shared history.

`pm_show.json` pins the internal planning snapshot used by Task resolution
and status refresh: a Project carries exactly one Wave Initiative and the repository
Team; its Task carries the stable Project and Team ids used for ownership.
The shared `LOO-*` identifier and canonical Project name remain presentation;
the provider's Wave-qualified title is normalized before this reader returns.
It is not a CLI/app DTO; Wave detail and roadmap fixtures cover those boundaries.

`task_execution_stalled.json` retains stalled Session evidence. `flow_process_progress.json` retains ordinary execution graphs and loop coordinates; run legality lives on the Task projection.

`task_files.json` pins `lf diff --files`, `lf diff`, `lf file` and `lf save` JSON:
exact comparison bases,
rename paths, scratch listing, and lossless content with a byte revision and
explicit file state, Save outcome, recovery access and late-change disclosure.
Rust round-trips the same fixture Swift decodes.

`task_checkout.json` pins the prepared Task receipt, including owning Machine and
resolved checkout. Rust round-trips the full Task snapshot; Swift reads the
workspace identity used before the next roadmap refresh. Flow graph fixtures
include the derived interaction stages and route references alongside steps.

`ask_session.json` keeps the caller capture separate from the Ask conversation.
A waiting Ask is available before its summary permits Complete.

`context_report.json` pins `lf usage --context --json`: each recorded step's
submitted input by source, flagged against budgets, with Task totals. One step
is LOO-298-shaped (384 steers over the goal budget); the other has no retained
capture, so every source is `null` rather than zero. Rust round-trips it and
Swift decodes it for the Task's Session history.

`desktop_open_explanations.json` pins proposed opening URLs, impediments and unknown
native state. Rust and Swift round-trip it; a proposal is not an opening receipt.

`desktop_openings.json` distinguishes request acceptance, usable endpoints and failures;
`desktop_inspection.json` requires top-level `openings` for failures before window
registration. Window receipts own Task/Session outcomes. Fixture shape is not native
readiness proof.

`task_run_explanations.json` covers edge take-up, no-Flow completion, ad-hoc Flow
selection and unavailable execution evidence. Explanation never grants admission.

`session_connect_explanations.json` preserves connection intent, preparation versus
takeover, shared legal actions and unavailable evidence without client acquisition.

`repository_identity.json` carries the selected plan and retained Machine-local locators.

`task_move_explanations.json` preserves exact prior Workflow positions, move/restart intent and unperformed completion checks.
