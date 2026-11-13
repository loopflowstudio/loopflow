# Launch recovery — 2026-11-13

Original report:

> (1) lf session resume session_4ce86dc10b614f56a02e453a41c3bbcb panics at rust/loopflow/src/agent/session.rs:301:51 with agent is a string, from checkout jack-heart/aa55a0-move-run-execution. (2) lf incident from checkout jack/loo-839-audit-every-sql-query-for-full-scans-and-long fails: invalid control authority: terminal planning state cannot start work; its execution history is preserved.

Before changes:

- Installed `lf`: `/Applications/Loopflow.app/Contents/MacOS/lf`. Recovery checkout: `jack-heart/incidentss`, clean at `295f257b8e`.
- `lf session show session_4ce86dc10b614f56a02e453a41c3bbcb --json`: saved cwd `/Users/jack/src/manabot.aa55a0-move-run-execution`, Task MTG-329, provider codex, model null, artifact key `3ed3fad8dbcc4f6c9aa12ea937ca2770`, recorded outcome completed.
- `lf config show --repo /Users/jack/src/manabot.aa55a0-move-run-execution --json`: effective agent `codex:gpt-6.1-sol`; selected candidate is personal/global `agents`, whose value is a YAML sequence. Resume assumes the candidate itself is a string at line 301.
- `lf task status LOO-839 --json`: Task `task_1f42ccce3b14446fba071f38b82325a9` done, PR #1905 merged, Task Graph `code` at `end`, final Graph process completed at 2026-11-13T02:16:20Z. Registered checkout `/Users/jack/src/loopflow.loo-839-audit-every-sql` still exists on the reported branch.
- Standalone named Graph dispatch inherits the checkout Task then calls `require_task_launch`, which rejects its terminal planning state. This is the admission used for starting Task work, even though `lf incident` does not move the Task's Graph.

Recovery changes:

- Resume reads the effective agent string after selecting its authored source; only an explicit scalar `agent` at CLI/environment authority switches provider.
- Standalone named Graphs retain checkout Task context without Task-run admission. `lf task run` and its carrier retain their admission and cursor behavior.
- No manual store repair, conversation-history edits, credential changes, installation changes or affected-worktree edits.

Verification (all Rust commands ran with inherited `LF_*` unset and stdin closed):

- `cargo test -p loopflow --test session_launch_contract resume_fallback_preserves_provider_and_can_refresh_its_model` — PASS (9.02 s): native Session token retained; another provider first in the list retains the saved model; the same provider first refreshes its model.
- `cargo test -p loopflow --test session_launch_contract task_conversation_preparation_is_passive_and_connect_uses_shared_launch_settings` — PASS (68.57 s, including a fixture CLI build): opening prepared Task input with an authored `agents` list launches the provider, then resumes the same native thread with current settings.
- `LOOPFLOW_TEST_LF=/Users/jack/src/loopflow.incidentss/target/debug/lf cargo test -p loopflow --test session_lifecycle_tests standalone_graph_launches_in_a_completed_tasks_checkout` — PASS (8.64 s): `lf -b --agent codex incident` launches a one-step fixture Graph with managed provider stand-in; Task attribution stays, Task stays done, captured Task Graph stays `code/end`, incident Graph succeeds.
- `cargo fmt --check` and `git diff --check` — PASS.
- `cargo clippy --all-targets -- -D warnings` — PASS (43.11 s).
- Test builds emit Apple's linker warning: `__eh_frame section too large (max 16MB) to encode dwarf unwind offsets in compact unwind table, performance of exception handling might be affected`; all focused tests pass.
- `target/debug/lf session resume session_4ce86dc10b614f56a02e453a41c3bbcb --prepare --json` succeeded against the live store: same Session ID, saved manabot cwd, codex provider. This is passive preparation, not a provider-start check. The returned command points to the Machine's installed gate binary, not this checkout build.

Recovery scope: the checkout build contains the fixes. The installed app and Machine gate still need the repaired build delivered through normal installation. No live provider was started or displaced during verification; end-to-end checks use the real public CLI with isolated provider stand-ins. Later incident steps can examine the originating changes and broader policy.

## Debug review — incident step 2

Cause trace for 5whys:

- `a5000ef889` (#1805) introduced resume's two reads of the selected candidate as a scalar. `112c27a053` (#1906) added `agents` and made its sequence a possible selected candidate in the effective `agent` explanation. The resolver still emits a scalar effective value from `Config::agent_order().first()`. The resume consumer retained the old authored-value assumption. This is a valid-input type mismatch, not damaged Session history or bad preferences.
- The remaining scalar `expect` is guarded by `candidate.key == "agent"`; that key is validated as a scalar. CLI/environment scalar choices may switch providers; lists and file defaults retain the saved provider. The recovery fix uses the effective value only for model refresh when its provider matches. No list iteration or provider migration is needed for native resume.
- `9c7b6828a9` (#1439) added Task launch admission to standalone dispatch when it inherits a checkout binding; `d20c56dafb` (#1503) moved that check into the store. The current rejection flows through `require_task_launch` → `require_task_planning`, before a named Graph records or launches a step. Task context was being treated as a request to start the Task's own Graph.
- Removing that call leaves checkout locks and Task interruption handling intact. Task cursor arrival remains conditional on a Task-run carrier; `run_task_graph` and Task-run preparation retain admission. The shared completion path still reconciles retirement for done Tasks; it does not reopen planning or rewrite Graph history. No additional production repair found.

Additional verification (isolated provider stand-ins, inherited `LF_*` unset, stdin closed):

- Extended `standalone_graph_launches_in_a_completed_tasks_checkout` to attempt `-b task run` afterward: it must refuse before another Session starts, with the Task projection unchanged. The first assertion expected the store error, but the public CLI correctly returned `Task … is done; … reopens planning without moving its Graph`; corrected the assertion. Rerun PASS (10.02 s).
- `cargo test -p loopflow --test session_launch_contract provider_switch_imports_session_history_with_current_policy` — PASS (7.92 s): an explicit scalar provider change still imports history with current policy.
- `cargo test -p loopflow --test session_launch_contract resume_fallback_preserves_provider_and_can_refresh_its_model` — PASS (9.47 s): lists retain native identity, keep the saved model for a different leading provider, and refresh it for the same leading provider.
- `cargo fmt --check` and `git diff --check` — PASS. No production edits in step 2; prior all-target Clippy result remains applicable to production code.

Launch-plan handoff: fixes remain only in this checkout. Publish and deliver the repaired build through the normal release/install path, then verify that both the installed app CLI and Machine gate resolve to that build. Installed-provider acceptance remains unverified: passive `--prepare` uses the installed gate, so it cannot establish repaired live resume. Any later live acceptance must retain the reported Session's native identity and completed LOO-839 Task's Graph/history; this step starts no live provider and changes neither record.

## Causal review — incident step 3

Status: both reported launch failures are repaired in this checkout, with the focused CLI results above. The affected workflows were continuation of the reported MTG-329 Session and incident investigation from completed LOO-839's checkout. The installed CLI and Machine gate remain unrepaired until delivery. This step inspected code, history and tests; it ran no providers, changed no live records and repeated no passing checks.

### Why resume failed, and why tests missed it

The history traced in step 2 shows a consumer contract changing underneath resume: #1906 allowed an authored sequence to be the selected candidate for inspection's scalar effective `agent`. Resume still treated that candidate as the resolved value. Both fields are YAML `Value`, so the compiler could not reject the mismatch; validation correctly accepted the list. The failure arose after valid configuration had resolved, before native continuation.

The test gap is specific: at `112c27a053`, `resume_fallback_preserves_provider_and_can_refresh_its_model` tried absent settings and scalar `default_agent` values only. New resolver tests checked list order, selected provenance and the scalar effective value, but did not pass that result through resume. The originating change did not update `agent/session.rs` or the CLI resume contract tests. Thus each side had coverage without exercising their new combination. There is no basis here to attribute this to credentials, damaged history or provider behavior.

Prevention already present: read the effective value for model refresh; retain authored key/source only for provider-switch authority. The added CLI cases cover both leading providers and retain the native token; prepared-Task continuation also uses a file-authored list. These directly exercise the missed combination without real credentials. No broad resolver redesign is justified by this single consumer mismatch.

### Why incident launch failed, and why tests missed it

#1439 added admission to standalone dispatch based on Task attribution, alongside the Task-workflow redesign. #1503 moved that check into the store without changing its placement. Attribution then required permission to start Task work even when no Task Graph cursor would move. Completion made that conflation visible: the recovery Graph was rejected before it could record or investigate anything.

Coverage did more than omit the completed-checkout case. The current `tests/linear/task_graph_launch_tests.rs:every_task_launch_runs_in_the_foreground_under_the_same_checks` groups Task-run, explicit Task-bound Graph and checkout Graph launches under identical admission expectations. Its `terminal` fixture is canceled planning input; it requires every entry to refuse and leave no Graph record. That encodes the same conflation. Other checkout-attribution coverage uses active Tasks. The new completed-checkout regression checks the missing distinction: standalone execution retains attribution and completion, while Task-run still refuses.

**Test repair identified for launch-plan (resolved below):** reconcile that existing admission matrix with standalone Graph semantics before publishing. Separate Task-run admission expectations from standalone execution; retain applicable checkout locks and test failures of commands inside a Graph separately from refusal to start the Graph. Inspect the other matrix conditions rather than deleting the test or making every launch succeed. Run the changed matrix's focused library test plus the completed-checkout regression after that repair. This was a test-contract conflict from source inspection, not a recorded failing run; earlier focused green results did not cover it.

### Limits and proportionate prevention

The common mechanism is a distinction lost at a shared interface: authored versus effective configuration, and Task context versus Task execution. The existing code already represents both distinctions; the repairs use them. A useful review practice is to follow a changed setting shape or admission rule into an existing continuation/recovery entry point and add one public-CLI case there. This is a proposal, not an additional implemented policy or a reason to add generic approval gates.

History and source establish the coverage gaps, but not which tests or review prompts ran before either merge. No claim about reviewer intent or CI execution follows. Installed-provider acceptance remains open; passive preparation cannot close it. No migration or Task-history repair is indicated.

Next useful action: launch-plan should include the admission-test reconciliation, then normal publication/delivery and installed CLI/gate acceptance described above. Further causal investigation has no identified payoff before those concrete gaps are closed.

## Execution decision — incident step 4

Keep both launch repairs, their regressions and the admission-matrix reconciliation in this checkout as one change. No Task, Wave, worktree or competing Graph is needed. Broader resolver redesign and review-policy proposals remain outside this repair.

The matrix now distinguishes Task-run refusal from standalone Graph execution for invalid planning, missing inventory, deleted input, canceled input, moved Project and wrong team. For standalone launches it checks the recorded command and that the recorded Graph outcome matches the command exit; a command failure is separate from launch refusal. The valid-input foreground/cursor/completion checks remain. Production checkout locks remain in the shared execution path.

Checks ran with inherited `LF_*` unset, stdin closed and the checkout CLI pinned for fixtures:

- `cargo test -p loopflow --lib task_graph_launch_tests::task_run_checks_admission_while_standalone_graphs_run_with_task_context` — PASS (27.01 s after build).
- `cargo test -p loopflow --test session_lifecycle_tests standalone_graph_launches_in_a_completed_tasks_checkout` — PASS (11.16 s): standalone incident succeeds, Task stays done at `code/end`, subsequent Task-run refuses without another Session.
- `cargo fmt --check` and `git diff --check` — PASS.
- `cargo clippy --all-targets -- -D warnings` — PASS (38.27 s).

Remaining delivery: publish this checkout's change through normal review/release/install, verify that the installed app CLI and Machine gate use the corrected build, then verify installed continuation and incident launch while retaining native Session identity and completed Task history. Installed software and live provider processes remain unchanged; this step establishes no live recovery. No further independently shippable prevention work was selected.
