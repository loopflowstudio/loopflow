# Open questions (LOO-974)

1. **Should managed Tasks deny writes outside the checkout?** The brief says
   "Keep the existing managed filesystem write boundary … writes elsewhere are
   denied." Nothing denies them today on any harness: the boundary's roots are
   never read, Claude runs with `--dangerously-skip-permissions` and Codex with
   `danger-full-access` (`rust/loopflow-execution/src/agent/mod.rs:617,729`).
   The plan keeps that behavior and deletes the unread descriptor. A real
   denial is an OS sandbox around all four harnesses and would be its own Task.

2. **Is deleting `AgentWriteScope` / `AgentExecutionBoundary` wanted, or only
   admitting OpenCode?** Chosen: delete. Flipping OpenCode's capability to
   `true` is the one-line alternative.

3. **Stop after one attempt when a step started no provider.** This also stops
   retries of transient pre-launch failures (a locked store). Chosen: stop;
   `lf task run` again retries.

4. **Idle steer wake for OpenCode and Pi work sessions** is in the plan as the
   last slice and may be better as its own Task.

5. **Limit fallback for OpenCode and Pi** is excluded, pending real failure
   payloads (LOO-518 for OpenCode).
