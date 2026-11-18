# Open questions (LOO-974)

Choices made for review; `scratch/concept-review.html` shows each one.

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

3. **A harness that is not installed, in a list.** Chosen: a Graph step passes
   it over and records the move, where limit moves are recorded. Passed over:
   filtering the order in `Config`, which would move chats too and say nothing.

4. **Stop after one attempt when a skill step started no provider.** This also
   stops retries of transient pre-launch failures (a locked store). `cmd:`
   steps keep three attempts. Chosen: stop; `lf task run` again retries.

5. **Idle steer wake for OpenCode and Pi work sessions** is out of this Task
   and needs its own. Nothing files it yet.

6. **Limit fallback for OpenCode and Pi** is excluded, pending real failure
   payloads (LOO-518 for OpenCode).

7. **A worker's question with nobody there** fails the turn with the question
   as the reason, on every harness. Passed over: declining the question so the
   turn continues. Larger, and not planned: the Task waits for the person.

8. **Should every headless run be `yolo`, not only those in a Task checkout?**
   Today a batch outside a Task checkout gets a floor (Codex `workspace-write`)
   and one inside gets full access. Not changed here.

9. **`Failed{resumable:false}`** is read and never written. Deleting the field
   touches a stored event shape and its DTO fixtures; not done here.
