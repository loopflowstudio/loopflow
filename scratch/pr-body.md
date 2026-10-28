A Task now runs its captured Graph directly. `interactive:` steps open in place under `lf graph`; `lf task run` stops before them so the Task conversation can take part, and the next run continues after that step. `-i`, `-b`, and `-a` override the step's launch choices. Machine selection remains per launch.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** [Graph steps choose their own harness and interactivity · LOO-622](https://linear.app/loopflow/issue/LOO-622/graph-steps-choose-their-own-harness-and-interactivity-or-the-docs-say)
> **PR lifecycle:** Merge delivers the code. Record independent later work as Tasks and comments; complete the delivered outcome.
<!-- loopflow:task-pr-context:end -->

`lf task run <ID> [GRAPH] --from <step>` replaces Workflow stage moves. The Task takes up its Wave's graph, then repository `graph:`, then builtin `code`, and retains that captured definition. `lf wave graph show/set` replaces the Wave Workflow commands. Ad hoc Graphs in the same checkout do not move the Task.

The duplicate Rust Workflow model, routes and move history are removed. One `task_graphs` row holds the captured graph, waiting step and current run's starting cursor/carrier. Task JSON and Swift models use `graph`; Desktop derives its workflow display from it, offers “Start from…”, and lists each Graph once in the command palette.

`lf ship` runs the remaining graph headless, including stand-ins at interactive steps. A blocked stand-in leaves the Task at that step, preserving earlier work. A failed run retries from its starting cursor. Per Jack Heart's October 27 direction, any Task run into `end` approves Auto-Ship and requests completion; this also covers an empty tail after the final interactive step and an XOR path that skips review. `lf ship --stop` withdraws approval. Reopened Tasks do not treat a previous completed delivery's merged PR as their new outcome.

The draft migration replaces `task_workflows`, drops `task_workflow_moves`, removes delivery `move_seq`, and renames the Wave default to `graph`. Released migrations remain unchanged. The parent-scoped process mark preserves Graph step recording when unrelated process rowids are reused and when `LF_BIN` is a wrapper.

Limits: old Workflow move history is not retained. `.lf/config.yaml` temporarily accepts `workflow:` so the installed release can still read repository configuration; LOO-637 removes that alias. Native provider interaction and subjective Desktop review remain separate from automated fixture coverage.

Validation:

- Released-migration history, architecture, Rust formatting, and Clippy pass.
- Python: 547 passed plus the separate checkout-observation proof. Desktop: app build, 629 headless tests and multiplatform boundaries pass.
- Website: 71 passed initially; the failed homepage-anchor test passes after correcting its link. All four documented-command checks pass.
- Focused materialized Rust: 244/246 passed, covering Graph/Task shipping, DTOs, CLI reference, migrations and Machine moves. The two failures were stale migration assertions against `task_workflows`; both pass after updating them to the finished draft schema. New XOR-preview and atomic end-approval regressions pass.
- The broad Rust run hit its 1,200-second budget after 933 passes and no assertion failures, including six minutes of cold compilation. The full Rust suite and unreached clone phase still require CI. Docker is unavailable locally, so the disposable installation upgrade proof also requires CI. Postflight resource inventory exceeded its 15-second uv-cache scan deadline; this gate is not an all-green full-matrix result.
