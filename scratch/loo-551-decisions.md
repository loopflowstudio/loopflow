# LOO-551 decisions (Jack Heart, October 21, 2026)

- Workflow keeps its name. Graph = authored definition; Workflow = a Task's nodes and edges, each edge running one Graph.
- Composition steps become `graph:` (`- graph: refresh`). Following the same noun, Workflow edges `flow:` become `graph:`. Migrate every repo and builtin YAML; no alias.
- Rename everything: CLI (`lf graph`), `.lf/graphs/`, Rust/Swift types (FlowDefinition, FlowProcess, FlowComposition…), JSON wire fields and DTO fixtures, and SQLite tables (flow_processes, flow_process_steps, task_flow_positions, work_flow_positions…) via the single draft migration.
- The musical "flow" in Loopflow product copy is unchanged.
- Shipped records stay as they are: release notes and applied migrations.
