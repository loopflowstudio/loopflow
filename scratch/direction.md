# Direction (LOO-974)

2026-11-18, Jack Heart on `AgentExecutionBoundary` / `AgentWriteScope` / `checkout_execution_boundary` / `supports_execution_boundary` after seeing that `writable_roots` is unread and `kickoff`'s plan deletes the boundary machinery instead of enforcing checkout write denial:

- "that is a weird thing to have built"
- "sounds good lets continue"
- "noting that there is an API opencode run that may be the right way to run oepncode headlesss"
- "did we move off the codex server" / "that is seemingly simiilar to opencode serve"
- "yeah lets do that"
- "Nah, bring that into this task"

Scope folded into LOO-974: align OpenCode launches with the split Claude and Codex use — `opencode --standalone` directly in the terminal for bare chats (no companion thread), `opencode run --standalone --format json --auto` for headless batches and Task workers, and `opencode serve` + `--server` kept only if `MachineServer` uses it to wake idle OpenCode TUI work sessions on steers (otherwise deleted too), removing obsolete `opencode serve` HTTP/SSE machinery.

2026-11-19, Jack Heart at `human-review` (`scratch/human-review.html` questions 1–3):

- "1a 2b 3b"

Meaning against the numbered options on `scratch/human-review.html`:
- **1a:** Keep stopped/interrupted worker reruns starting a fresh conversation on the next `lf task run` (workers stay Runs; steers carry answers forward).
- **2b:** File follow-up Tasks so every `no` cell in `rust/loopflow-execution/src/harness/README.md` names a Task — filed in `infrastructure`:
  - `540073`: Wake idle OpenCode and Pi work sessions when steers arrive
  - `98edc6`: Classify Pi usage-limit and transient failures so Graph steps fall back and wait
  - `48edb9`: Fail headless Claude and Codex worker turns when they ask a question with nobody there
- **3b:** Do one more `build` pass on this branch (Slice 8 in `scratch/plan.md`) to delete uncalled `Driver::send_current`, read `unanswered_question` from the recorded turn in `history.turns` instead of splitting `"unanswered_question: "` out of step error prose (`asked_question`), and name the filed follow-up Tasks in `harness/README.md`'s parity table.

2026-11-19, Jack Heart at `human-review` (pass 2, `scratch/human-review.html`):

- "land it"


