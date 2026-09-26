# Task Monitor panes — first increment, 2026-09-24

Finish: selecting a Task reveals a retained Monitor in the existing multiplexer;
Session/companion panes survive, exact typed Task Runs render, unavailable evidence
cannot become confirmed emptiness, and closing observation cannot stop a provider.
Model assignment alone does not prove native surface/input retention.

This first increment uses demand refresh through one Podium Home reading. The
reviewed shared active-Run reader still traverses retained native receipt
directories. Adding a frequent poll now would cross the recorded cost boundary.
Refresh on Monitor opening and an explicit Refresh button provide useful current
observations; each pane shows the observation time. Bounded discovery and automatic
refresh remain core work before the complete canvas ships, alongside both native
performance runners, baseline/budgets and the simplified human demonstration.

The two already-defined performance endpoints remain hierarchy_interaction_ms
and task_workspace_ready_ms. This increment claims no latency or frame-hitch
measurement. Their before/after runners must exercise these actual mixed panes.

Existing dirty files contain concurrent outline and active-Run contributions.
They are preserved. No blanket checkpoint/staging claims another writer's work.

## First proof and counterexamples

The first focused run compiles and passes Task-filter/stale/gap/empty presentation
and mixed-layout behavior, but fails native focus: after selecting Monitor,
AppKit first responder remains the retained shell terminal. Draft and companion
responses still pass. `/tmp/loo291-task-monitor-proof.log`.

An explicit window-local `clearFocus` attempt also fails that assertion; do not
claim it restores the input boundary. Concurrent independent proof reproduced
the same failure for direct Session panes and a separate saved-pane reuse bug.
That writer's `PaneState` selection validation is preserved. They own the native
Monitor focus target correction; coordination is in monitor-proof-coordination.md.

The corrected-build attempt initially hit a Swift compiler assertion in the
concurrently added proof's optional closure invocation. Its writer separated
the invocation; the following build compiles. This is not a product-test pass.

Fallback compilation via `uv run python scripts/test.py --loopflow` stopped at
resource preflight: main's active build root is 19.1 GiB against a 12 GiB budget.
Supported `scripts/resource_envelope.py --recover` preserves that active root
and reports the same block. No compiler verdict, bypass or broad suite run is
claimed. Logs: `/tmp/loo291-monitor-{fallback,resource-recovery}.log`.

## Final outcome

The first increment is implemented. Task rows restore a saved pane or open a
Task-bound Monitor in the existing checkout multiplexer. Monitor adds to occupied
layouts and never replaces their only terminal reference. Upcoming Tasks use the
repository workspace without creating a checkout. Sessions menus/outline leaves
select exact existing conversations; contextual Inspect retains planning access.
Podium owns one demand-refreshed Home reading across all panes. Each Monitor
resolves its planning Task to shared runtime Work, filters exact identity, shows
observation time and preserves stale/incomplete distinctions.

The concurrent writer corrected two reproduced failures: saved choices now capture
existing PaneState content as well as location; Monitor has an AppKit responder
that accepts its pane's focus request. Clearing to nil was insufficient because
AppKit could focus a visible terminal. That failed attempt and all clearFocus
methods/call sites were removed. Existing terminal focus behavior is unchanged.
The direct Session proof belongs to that writer; the shell-attached proof is this
turn's complementary coverage. No duplicate Monitor reading/layout owner exists.

Final command in [receipt](monitor-evidence/receipt.json): **six tests pass**.
The real PTY tests require exact unfinished input plus the child's reply, companion
responses, unchanged native surfaces, actual first responder, zoom/resize and Task
return. Model/view proof covers typed Task isolation, unchanged Monitor subject
when selection changes, failed reads, incomplete/empty distinctions and shared
split/Close/Undo/reconciliation. [Final log](monitor-evidence/final.log).

These are mounted native fixtures, not vendor-provider interaction or human UI
approval. The pending fallback build remains a resource-preflight failure with
no compiler verdict; supported recovery retains main's active build. No broad
suite, installed app change, provider transfer, PM mutation, publication or Task
completion was performed. Whitespace-only source cleanup followed the final pass.

Review: identity and presentation remain separate; a selected Task cannot redirect
a retained Monitor or restore another Task's replacement Session. Monitoring owns
no process actions, and selecting/closing a Monitor launches/stops nothing. No
per-pane poller, output tailer, second workspace or Task lifecycle is introduced.

Next core work remains bounded live discovery and automatic shared refresh,
then the two defined native performance journeys and comparable baselines/budgets.
The user still needs to confirm the simplified composition; prior demos do not
satisfy that boundary. LOO-293 and both optimization follow-ups retain their
recorded dispositions; nothing was closed or created as a shortcut.
