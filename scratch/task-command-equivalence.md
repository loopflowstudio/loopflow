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
| Current binary starts Flow children | Explicit recursive lock through PATH → ordinary PATH → selected installation → driver executable | PATH honors the lock's leading directory; missing PATH lf falls back without using historical control pins. The child's Exec command records its executable path. Lock creation remains LOO-334's responsibility |
| Child migration followed by old-driver settlement | Read-only schema compatibility check before any Flow settlement | Implemented; incompatible driver exits with selected result retained |
| In-turn `lf flow blocked` command | Selected successful structured result with a required reason | Removed; keyed Ask completion starts another turn in the same conversation |
| Task worker claim and managed Flow selection | Driver admission/settlement | Retained mechanics; no longer select step permissions |
| `agent_work_in` supplies calling Session Task | Explicit declaration or checkout | Deleted; causal parent lookup stays |
| `LF_TASK_ORIGIN`, claim/manifest-derived Task origin | Explicit declaration or checkout | Deleted writers/readers; installation and PR guards use the common reader. A missing registry plus no declaration is unmanaged, regardless of caller input |
| Flow's captured Task names a new step | Common declaration/checkout resolution before reservation | Step prompt identity resolves independently and must agree with retained Flow membership; persistent membership/provenance remains part of the owner migration |
| Task worker starts descendants under its Task | Same explicit `LF_AS` declaration as `--as` | Shortcut declares the selected Task; claim still fences writes only |
| Provider/tmux/SSH/handoff forwards Task origin | Forward the unchanged explicit `LF_AS` declaration | No Session, claim or checkout writes it; Y checkout work can retain ancestor declaration X for later descendants |

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

## Declaration precedence · Jack Heart, 2026-09-30

Explicit selection on this command → Task checkout → ancestor declaration.
Jack replaced the earlier operator assumption that put inheritance ahead of
checkout. `LF_AS` is the single declaration variable; no existing variable carried
that contract. Legacy `--task`/`--wave` selection uses the same declaration.
Task execution shortcuts explicitly select their target, including a new Y from
inside X's agent. Existing conversation binds remain durable and prospective;
neither a bind nor a caller Session manufactures a declaration.

Provider replacement still changes the causal parent resolver's current-driver
choice. A stale caller retains its original Exec parent; after deleting Work
inheritance it can issue an ordinary unbound command outside Task checkouts.
That command acquires no authority to settle the old Flow or Session. The previous
blanket launch refusal was a side effect of inheriting Work, not a causal-tree rule.

The pass migration, dead `caller_flow_turn` column, retained Flow membership
provenance and separate recorder naming cut remain open. Current scope deletes
new Task attribution through parent Sessions, claims and Task-origin bytes.
