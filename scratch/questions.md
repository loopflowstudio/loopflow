# Open choices — LOO-446

2026-10-09 implementation choice: the migration rewrites `driver:<n>:exit`
receipt keys instead of reading both prefixes, following Jack Heart's stated
preference for no compatibility layers. Payloads keep their recorded bytes,
including `"type":"driver_exit"`; no reader consumes that field.

2026-10-09 implementation choice: `attached_lf_process_id` and
`agent_sessions.agent_process_id` are renamed inside LOO-443's unreleased
`agent_process` draft, because a second draft may not alter what an unreleased
draft created. A sync onto a moved LOO-443 will conflict there.

2026-10-09 assumption: the Codex FIFO is now `attachment.lifeline` and the
socket `agent.sock`. A Codex AgentProcess started by an earlier build keeps its
old FIFO; a later attachment finds none and treats it as a launch without a
lifeline, so that provider ends with its original lf. Live journal receipts
written by an earlier build carry the old JSON key and are unreadable to this
one. No reader for either old name was kept.

2026-10-09 left in place, outside the rename: two captured terminal-host trial
receipts (`docs/reviews/terminal-host-trial/final-02-*.jsonl`) and a Cursor
quotation in `swift/DESIGN.md` still match the Task's `rg` check; the released
index name `session_driver_exit` remains in the dated storage review; and
`scripts/bump_patch_version.sh` still classifies commit subjects containing the
old word. Each is a record or a classifier of history, not vocabulary in use.

2026-10-09 unverified: `tests/e2e/codex_connect.py` assumes an AgentProcess row
clears `endpoint` and `attached_lf_process_id` when its owner exits, and that
its `parent_lf_process_id` is the headless lf that launched it.

2026-10-09 doc facts not rechecked: `docs/architecture/data.md` still describes
the Session's attachment as a nullable Process reference with a generation
fence, and TESTING.md says fixtures carry "attachment and provider generations".
Only the vocabulary changed; LOO-443 owns the attachment-token wording.

Signal worth sponsoring: a CI check that runs the Task's `rg` over the tree
would keep the retired words from returning; it costs one line in lint.
