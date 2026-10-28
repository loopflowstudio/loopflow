# DTO wire fixtures

Each fixture pins one live wire shape. Swift fixtures cover the per-Wave
listener and `lf wave status` contracts consumed by the Mac app. Every absent field
is a parse error or an explicit null.

`task_detail.json` pins `lf task status --json`: one Task row (each fact once:
state, reason, Workflow stage, PR, next command) with its Sessions, Graph
processes and comments. `lf task list`, `lf wave show` and `lf wave status`
print the same row. Rust round-trips it; Desktop reads `lf desktop` instead.

`task_status.json` pins `lf desktop task --json`: the planning/execution envelope
for available, unavailable, invalid, removed and absent planning, with the Task's
overview, Graph processes and comments. Rust round-trips it. The Mac app reads the
overview, Graph processes, comments and recorded work from
`lf desktop task --watch` (`watch_frames.json`). These cases have no execution.

`watch_frames.json` pins one `--watch` frame per read a window runs:
`{answers, unavailable, body}`, where `body` is what the
read prints with `--json`, or `null` with `unavailable` set. Rust checks each body
against its read's type; Swift decodes each through its part.

`move.json` pins the Session or Task move `lf session send` prints and `lf
session receive` reads on another Machine. Only lf reads it, so it has no Swift mirror.

`task_execution.json` pins the execution boundary inside the status envelope's
optional Task snapshot (`execution.execution`).
This CLI-only contract is tested in Rust. The Mac app receives its derived
condition, reason, and actions through the existing Wave Task row.

`session.json` pins `lf connect --prepare --json`: one unresolved Task GraphStep
session, its explicit readiness state, and its exact Machine-local attach route.

`session_memberships.json` pins each Session's required `graph_membership`:
the current Graph step, an earlier step, an independent conversation, a Session that
predates recorded membership (unknown, never independent), and a remote Graph
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
The optional `task_pr_id` retains the PR captured by the managed Graph;
`first_provider_attempt_at` differs from capture/import observation time.

`work_activity_snapshot.json` pins `lf history --json`: durable Work creation,
Session capture/provider history, PR and Steer facts retain their original
identities. Rust and Swift round-trip this shared history.

`task_execution_stalled.json` retains stalled Session evidence. `graph_process_progress.json` retains ordinary execution graphs and loop coordinates; run legality lives on the Task projection.

`task_files.json` pins `lf diff --files`, `lf diff`, `lf file` and `lf save` JSON:
exact comparison bases,
rename paths, scratch listing, and lossless content with a byte revision and
explicit file state, Save outcome, recovery access and late-change disclosure.
Rust round-trips the same fixture Swift decodes.

`task_checkout.json` pins the prepared Task receipt, including owning Machine and
resolved checkout. Rust round-trips the full Task snapshot; Swift reads the
workspace identity used before the next roadmap refresh. Compiled graph fixtures
include the derived interaction stages and route references alongside steps.

`ask_session.json` keeps the caller capture separate from the Ask conversation.
A waiting Ask is available before its summary permits Complete.

`context_report.json` pins `lf usage --context --json`: each recorded step's
submitted input by source, flagged against budgets, with Task totals. One step
is LOO-298-shaped (384 steers over the goal budget); the other has no retained
capture, so every source is `null` rather than zero. Rust round-trips it and
Swift decodes it for the Task's Session history.

`work_links.json` pins `lf open --link --json`: a readable label, the public Work
address, its executable fallback, the resolved target (including a remote execution
owner), the caller's conversation relation and the proposed route. Rust and Swift
round-trip it; Desktop resolves incoming addresses through it. A link is not an
opening receipt.

`desktop_openings.json` distinguishes request acceptance, usable endpoints and failures;
`desktop_inspection.json` requires top-level `openings` for failures before window
registration. Optional window `view` contains the same shared selection plan,
including hidden/suppressed occurrences. Window receipts own Task/Session outcomes. Fixture shape is not native
readiness proof.

`task_run_explanations.json` covers edge take-up, no-Graph completion, ad-hoc Graph
selection and unavailable execution evidence. Explanation never claims ownership.

`session_connect_explanations.json` preserves connection intent, preparation versus
takeover, shared legal actions and unavailable evidence without client acquisition.

`repository_identity.json` carries the selected plan and retained Machine-local locators.

`repo_people.json` pins `lf ops people --json`: each Task owner's claim email and
name, with a GitHub identity that is null when unresolved.

`task_move_explanations.json` preserves exact prior Workflow positions, move/restart intent and unperformed completion checks.

`task_planning_explanations.json` preserves creation, edit revision/fields, comment
append versus provider refresh, effects and explicit refused input.

`task_checkout_explanations.json` retains proposed, reused and restored checkout intent separately from admission and unavailable evidence.

`task_locations.json` covers request-bound Machine observations, retained paths,
pathless execution, local absence and unavailable peers. Negative readings grant
no ownership; execution locations never enter planning exchange.

`task_files_frames.json` preserves request/Task/Machine/checkout identity and distinguishes filesystem invalidation from heartbeat.

`dashboard.json` pins an independently identified Dashboard and its expression
and arrangement; Task panes and drafts remain Work-owned. Rust and Swift round-trip
it without manufacturing defaults.

`view_plan.json` pins the shared headless selection/arrangement plan, including
repeated Work, duplicate suppression and a backlog Task without a primary Session.
Rust round-trips it; Swift decodes the same plan directly and from `lf desktop plans --watch` in `watch_frames.json`.

`pane_layout.json` covers portable pane content, focus versus explicit zoom, hidden
members, two Shell tabs, two Files tabs, selected-tab identities and file-draft
base recovery. Shell identity is independent of its tool pane; saved intent never
asserts a live native process. `view_plan.json` includes query discovery
metadata, independent Dashboard identity and portable tile-face intent.

`session_rows.json` pins `lf session list --json`: one row per Session, each
fact once, every field required. Desktop reads `lf desktop sessions`
(`sessions.json`).

`closed_task_page.json` pins `lf task list --done --json`: one page of closed
Tasks as `lf wave status` Task rows, most recently closed first, and the `next` Task to pass as
`--after`. Open Tasks come from the plan; closed ones only from these pages.

`retention_summary.json` pins `lf history retention --json`: Sessions by harness,
account, age and retention state (`provider`, `facts`, `unknown`), the soonest
removals ahead, and each home's expiry policy. Rust and Swift decode it; the
Desktop Retention card renders it.

`task_create_receipt.json` pins `lf task create REQUEST --run --json`: the created
Task, the graph it took up and its admitted primary conversation, null after a
headless `-b` run; both are null when the run failed after the Task was filed. `wave_graph.json` pins `lf wave graph show --json` for a Wave
and for the repository (`--no-wave`); `default` is Rust's resolution, which
Desktop's New Task sheet shows instead of rebuilding it. Rust and Swift decode both.
