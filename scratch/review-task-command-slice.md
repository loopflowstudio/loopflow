# Task command slice review · 2026-09-30

LOO-298 · Jack Heart · Reviewed `36cbb3d4b..b341ed3c5` against
[the accepted model](data-model-one-table-per.md) and
[the command audit](task-command-equivalence.md). This review covers the complete
two-commit slice, its callers and its public execution paths. It does not repeat
the entire branch's concept review or establish configured acceptance.

## Evidence matrix

| Claim | Planned owner | Implemented behavior | Evidence | Result |
| --- | --- | --- | --- | --- |
| Attribution has one source | `WorkBinding.source` | All three bound launch callers use the resolved field; explicit and channel selection remain Declared, checkout selection remains Checkout | Caller inspection; three fresh public CLI attribution tests | Pass |
| Worker startup preserves selected Task and causal parent | Existing Task worker admission and Exec ancestry | Startup consumes its environment directly; the single-use forwarding function and clone are deleted | Fresh disposable Linux `declared_agent_can_start_another_tasks_flow` | Pass |
| Cross-Task fixture respects confinement | Common harness boundary policy | Y explicitly selects Claude; Y's mechanical Flow starts no provider. Interactive scripted OpenCode in X issues the command | Same public proof completes Y, checks Y's declaration and traces the operation through worker/command to X's AgentSession | Pass; scripted provider/terminal |
| Work context stays intact | Shared Work envelope formatter | Optional message appends to the same context, preserving whitespace behavior | Complete diff inspection; retained 31-test prompt/binding pass at the reviewed source | No regression found |
| One started Flow has one FlowSession | Root graph, cursor and iteration tuple | Runtime child rows and their claims still exist | Negative architecture inspection of `store/sqlite/flows.rs` and the child migration | Unfinished; outside this slice |

## Fresh proof

Resource preflight passed with 35.1 GiB free against the 32 GiB reserve.
The runner removed inherited `LF_*`/`LOOPFLOW_*` authority and used disposable
data. No branch binary accessed installed data.

```sh
uv run python scripts/test_task_installation.py --test declared_agent_can_start_another_tasks_flow
cargo nextest run -p loopflow --test session_cutover_tests -E 'test(interactive_run_records_checkout_and_declared_work) | test(provider_parentage_does_not_assign_work_outside_its_checkout) | test(declared_agent_tools_use_their_checkout_and_keep_the_exec_parent)' --test-threads 3
```

The installation harness passed its one selected test in 2.20 seconds after
compilation. Docker responded with 29.4.0; the harness removed its disposable
container on successful exit. This closes the earlier local execution gap for
the corrected fixture, not the rest of the installation suite. Log:
`.lf/tmp/cut-i/review-slice-task-shortcut.log`.

The three public attribution tests passed in 13.098 seconds. They exercise the
real CLI with scripted OpenCode and inspect persisted attribution and ancestry:
explicit/checkout/unbound selection, checkout precedence over inherited
declarations, and stale provider provenance without inherited Work. Log:
`.lf/tmp/cut-i/review-slice-public-attribution.log`.

Existing compression proof is retained without rerunning: 31 prompt/binding
tests and all-target Clippy with warnings denied passed on the reviewed source
(`compress-binding-proof.log` and `compress-binding-clippy.log` beside these
logs). The earlier complete materialized matrix and hosted Rust/Swift reports
remain historical evidence; this review ran four focused tests, not those suites.

## Findings and next action

No bounded production defect was found in this slice. Removing the independent
source argument makes the launch API agree with its resolved binding; consuming
the worker request removes a needless copy without adding another owner. The
production diff removes 26 net lines, with three fixture lines added.

Negative searches confirm `TaskLauncher`, `SavedLauncher`, `start_work_session`
and `agent_work_in` are absent. Remaining `LF_TASK_ORIGIN`/`LF_PARENT_RUN_ID`
mentions are sanitation or fixture evidence, not restored identity writers.
`caller_flow_turn` remains on Exec reads/import evidence. Runtime child
creation, recursive root/current-child reads and claim transfer remain in
`store/sqlite/flows.rs`; the child index/view/triggers remain in migration SQL.
Those are outstanding reductions, not a hidden completion claim.

Next implementation: the forward child-pass migration and removal in
[remaining work](remaining-work.md), preserving exact history/completion
references and resumable review state. Retain managed account/control proof
gaps, released-populated import, Chapter integration and configured acceptance.
The supplied `scratch/concept-review.md` reference has no file; use
[concept-review-findings.md](concept-review-findings.md) and the accepted model.
This review recommends publishing the coherent slice. It chooses no Flow edge
and authorizes no Task completion or merge.
