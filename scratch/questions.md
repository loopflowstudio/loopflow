# Open questions (LOO-974)

Choices made for review; `scratch/concept-review.html` shows the open ones
as the change stands after QA. Slice 7 of `scratch/plan.md` takes up 19, 21,
22 and 25; 18 stays unread and the README says so.

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
    `lf steer LOO-12 "…"` and `lf task run` again. The reason on OpenCode and
    Pi is now "<harness> asked a question with nobody there to answer it:
    <question>" (Pi's "open this Session in Pi to answer it" is gone). Is
    steer-then-run the answer path, and should the reason print that command?

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

15. **Steers to a running headless OpenCode turn** rely on the plugin's
    between-tool check alone. Seen working on OpenCode 2.2.1 in QA
    (2026-11-18) once the plugin resent each steer on every later request.

16. **A missing agent in the middle of the order is skipped without a record.**
    Only a missing first choice shows as a move in `lf task status`; a later
    one is taken out of the order before the step starts. Recording it would
    need a second row per step or a longer reason.

17. **With `agents: [opencode, codex]` and OpenCode missing, a refused Codex
    step's stop advice reads "Nothing is left in its order after codex"**; it
    does not name the entry that was passed over.

18. **What does a headless Claude or Codex do with a question?** The parity
    table's question row now reads "not shown" for both. No decoder fails a
    turn on Claude's `AskUserQuestion` or Codex's `requestUserInput`
    (`codex_decode.rs` only marks the Session as waiting), and no trace of
    either was recorded. The orchestration doc's sentence ("a headless step
    whose agent asks a question fails with the question as its reason") is
    shown for OpenCode and Pi only.

19. **A question is asked three times.** The failed turn is a failed step, so
    `lf task run` starts it again twice and the model asks again each time
    (`a_question_with_nobody_there_ends_the_step` takes 72s for that reason).
    Stopping after the first question needs the retry loop to tell a question
    from another failure. Chosen for review: slice 7 stops after one, by a
    code on the failed turn.

20. **Pi's question text is read from `title` and `message`** of its
    `extension_ui_request`, taken from memory of Pi's RPC shape and the
    stand-in; no real Pi dialog was recorded. A request with neither gives the
    reason without a question.

21. **"Its stopped Task Session resumes" was built as "the next `lf task run`
    starts the step again".** A worker's Session is a Run and `lf resume`
    refuses Runs, so the interrupted conversation's native thread is never
    continued; the step gets a new one. The test keeps the name the docs gave
    it; slice 7 renames it to what it shows. Should an interrupted step
    continue its own conversation instead?

22. **What does an interrupted Claude or Codex worker record for its turn?**
    Not read. The test runs OpenCode and Pi; the README says "not shown" for
    the other two. On OpenCode and Pi the turn was recorded `failed` until
    this slice. Slice 7 adds both to the test.

23. **Ctrl-C in a terminal reaches the provider and lf together.** If the
    provider dies before lf's handler runs, `agent::interrupted()` is still
    false when the reader settles and the turn can read `failed`. The Graph's
    stop signals lf alone, so a Task step does not race.

24. **The worker tests are slow only on the first run after a build**
    (120-175s against 39s). Not looked into; nextest's limit is 300s.

25. **A Pi worker's usage is missing from `lf history usage --task`.** The
    session file's turn id (`rpc-user-1`) and lf's request id name the same
    turn in two `history.turns` rows; usage sits on the one with no Task.
    Seen on the stand-in only. Fix candidates: name the Pi turn after the
    session file's user entry, as OpenCode's driver does with its store row,
    or have the transcript read attach usage to the recorded turn that
    `covers` it, as it already does for completions. Chosen for review: the
    second, in slice 7. QA's real RPC runs left session files under
    `~/.pi/agent/sessions/--private-tmp-loo974-*--/`, and real Pi showed the
    same missing usage.

26. **`Driver::send_current` has no production caller.** Claude, Codex and Pi
    implement it and test it; steers reach a running turn only through the
    hook after a tool. Delete it, or is same-turn input coming back?

27. **`lf history usage --task` lists the Task's interactive `task-session`
    beside the worker**, with no usage. Not looked into.

28. **A person's own `OPENCODE_CONFIG_CONTENT` is no longer merged.** lf read
    it from its own environment, where inside an OpenCode conversation it is
    that conversation's, plugin included. It is now a caller marker; OpenCode's
    config files still apply. Passed over: telling a person's value from a
    caller's.

29. **A missing agent's move shows in `lf task status` only while the step
    runs**, as a limit move does. After the step settles, the row stays in
    `agent_limit_recoveries` and nothing shows it.

30. **With `agents: [pi:…, opencode:…]` and Pi missing, no Task conversation
    started**, and status reads `body: provider pi`. Chats do not move, so the
    conversation is not opened on OpenCode. Should it be?

