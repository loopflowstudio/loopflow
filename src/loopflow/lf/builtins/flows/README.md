# Built-in Flows

Flows shipped with loopflow. Available everywhere without user configuration.

| Flow | Steps | Use case |
|------|-------|----------|
| ship | design → implement → polish | Full feature workflow |
| quick | implement → polish | Fast iteration |
| iterate | review → implement → polish | Improve existing code |
| reduce | reduce → polish | Simplify bloated code |
| roadmap | roadmap → design | Strategic planning |

## Adding a Built-in Flow

1. Create `{name}.py` with a `flow()` function
2. Return a `Flow` with steps
3. Update this README

Fork failure handling is undefined for v1. Document edge cases here as they're discovered.
