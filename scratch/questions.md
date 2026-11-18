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

10. **How is a worker's question answered?** `docs/orchestration.md` now says:
    the step fails with the question as its reason; answer with
    `lf steer LOO-12 "…"` and `lf task run` again. Pi's message today says
    "open this Session in Pi to answer it", which `lf resume` cannot do for a
    headless Session. Is steer-then-run the answer path, and should the
    failure reason print that command?

11. **The parity table's "no" cells name no Task** for idle steer wake
    (OpenCode, Pi) or Pi limit classification; only LOO-518 exists. File them,
    or leave the cause in the cell?

12. **Do OpenCode work sessions need a server at all?** Chosen: no. Every
    OpenCode terminal launch is `opencode --standalone`; nothing keeps
    `opencode serve` or `--server`. Idle steer wake for OpenCode (question 5)
    would bring one back, or use `opencode api` against the TUI's own server.

13. **Should a failed OpenCode start print its reason?** It now prints only the
    exit status; the reason sits in `opencode.stderr.log`. Printing the log's
    last line on a failed start is the alternative.

14. **`tests/fixtures/dto/work_first_entry.json` still quotes the deleted
    refusal** ("does not support a managed execution boundary") as an OpenCode
    Task's reason. Regenerating it (`LOOPFLOW_UPDATE_WORK_FIRST_FIXTURE=1`)
    also rewrites key order, new fields and checkout paths that drifted before
    this Task, and reaches the Swift fixture tests. Left as committed.

15. **Steers to a running headless OpenCode turn** now rely on the plugin's
    between-tool check alone. Not seen working on the real CLI yet.

16. **A missing agent in the middle of the order is skipped without a record.**
    Only a missing first choice shows as a move in `lf task status`; a later
    one is taken out of the order before the step starts. Recording it would
    need a second row per step or a longer reason.

17. **With `agents: [opencode, codex]` and OpenCode missing, a refused Codex
    step's stop advice reads "Nothing is left in its order after codex"**; it
    does not name the entry that was passed over.
