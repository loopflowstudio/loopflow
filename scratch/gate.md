# LOO-880 gate

Contract: kickoff → write-docs → implement → concept-review → refresh (sync, realign) → loop-or-next → compress → qa → loop-or-next → pr-publish; both decisions return to implement.

Verification plan: graph YAML and graph expectations reach graph, graph_tests, graph_discovery_tests, task_graph_launch_tests and world_recipes; embedded launch-plan reaches golden_prompt; Markdown reaches documented_commands and website. The changed-aware runner also selects architecture, formatting and Clippy. No UI or performance measurement applies to this ordering-only change. Embedded resources require cargo package verification after the runner.
