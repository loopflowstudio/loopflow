# Built-in Flows

Flows shipped with loopflow. Available everywhere without user configuration.

| Flow | Structure | Use case |
|------|-----------|----------|
| ship | implement → reduce → polish | Full feature workflow |
| roadmap | Fork(roadmap×3, synthesize) | Strategic planning with multiple perspectives |

## Adding a Built-in Flow

1. Create `{name}.py` with explicit import: `from loopflow.lf.flows import Flow, Fork`
2. Return a `Flow` with steps
3. Update this README

See `src/loopflow/lfd/execution/README.md` for Fork execution details.
