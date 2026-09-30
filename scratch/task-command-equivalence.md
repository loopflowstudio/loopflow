# Task commands and ordinary execution

LOO-298 · Jack Heart's 2026-09-30 decisions. Working implementation audit;
this is not acceptance or a completed attribution cut.

`lf task run X` adds managed Flow selection, claim fencing and background
placement. Those driver mechanics must not change the step's prompt, provider
behavior or permissions compared with the same Flow in X's checkout.

| Former Task-only branch | Current owner/key | Status |
| --- | --- | --- |
| Task seed in `run_flow_skill` | Shared `build_prompt_at`, explicit Task selection or existing checkout binding | Implemented; direct skill and ordinary Flow use `ops/task_input` for seed, steers, interrupts and attachment |
| Task lookup requires active PR / undeleted Task | Existing checkout binding and latest historical PR | Removed from context resolution; mutation eligibility remains with its operation |
| Name supplied separately by launch callers | `gather_context` calls `launch_user_name` | Implemented for shared prompt preparation; public scripted three-path proof passed |
| `claim.is_some()` sets worktree scope, execution boundary and skipped permissions | One `confine_checkout_agent(task_checkout, interactive)` predicate | Implemented for the intersection: unattended execution in a Task checkout. Both direct and managed commands use it |
| Task-specific account preflight error classification | `provider_account::preflight_agent_account` and shared agent selection | Moved; inherited account leases and shared failover retain their owners |
| Task-specific writable roots and probe | Agent execution boundary | Moved; Task preflight delegates before allocating work |
| Current binary starts Flow children | PATH resolves `lf` at each step | Implemented; recursive executable lock remains LOO-334's responsibility |
| Child migration followed by old-driver settlement | Read-only schema compatibility check before any Flow settlement | Implemented; incompatible driver exits with selected result retained |
| In-turn `lf flow blocked` command | Selected successful structured result with a required reason | Removed; keyed Ask completion starts another turn in the same conversation |
| Task worker claim and managed Flow selection | Driver admission/settlement | Retained mechanics; no longer select step permissions |
| `agent_work_in` supplies calling Session Task | Must become `--as` / checkout | Outstanding item 3; causal parent lookup stays |
| `LF_TASK_ORIGIN`, claim/manifest-derived Task origin | Must become `--as` / checkout | Outstanding item 3, including managed PR and installation guards, cross-Home forwarding |
| Flow's captured Task names a new step | Must resolve step checkout or explicit selection | Outstanding attribution audit; do not confuse retained Flow membership with a new Task identity source |

## Confinement decision still open

Jack requested one predicate without choosing the wider policy. The current
intersection preserves confinement for the previously managed unattended case
and makes the equivalent direct command agree. Neither wider option is selected:

- Key on the Task checkout: also confine interactive skills and reviews there.
- Key on unattended execution: also confine headless work outside Task checkouts.

The choice should change only that predicate. Account routing and filesystem
capabilities remain provider/process concerns, not Task failure classifiers.

## Provenance consumer

`ConcreteSkill.flow_parents` has a real consumer: compilation copies it into
`FlowGraphNode.parents`, and Swift `TaskFlowView` displays `from …` breadcrumbs.
Jack requested reporting consumers instead of renaming the field. Removal must
include an explicit choice about that display; no substitute name is introduced.

## Proof boundaries

The PATH public test runs the actual CLI against a disposable store. Its newer
schema is injected by a PATH wrapper after a successful mechanical command; it
proves refusal and retained effects, not interoperability with a separately
released binary. The broader public fixture uses real lf and scripted Linear,
Codex and tmux in disposable Linux. Real configured acceptance remains open.
