# Open choices — LOO-446

2026-10-09 implementation choice: the migration rewrites `driver:<n>:exit`
receipt keys instead of reading both prefixes, following Jack Heart's stated
preference for no compatibility layers. Payloads keep their recorded bytes,
including `"type":"driver_exit"`; no reader consumes that field.

2026-10-09 implementation choice: `attached_lf_process_id` and
`agent_sessions.agent_process_id` are renamed inside LOO-443's unreleased
`agent_process` draft, because a second draft may not alter what an unreleased
draft created. After #1519 merged, the same holds for the columns it added
(`caller_agent_process_id`, `agent_process_id`): main's landed, unreleased draft
is edited in place. Draft-bearing builds never promote, so only a disposable
store that applied main's text keeps the old column names; recreate it. Jack
asked that migration work stay cheap.

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

2026-10-09 doc repair outside the rename: `docs/architecture/data.md` and
TESTING.md described the attachment's generation fence, which LOO-443's draft
replaces with `attachment_token`. Both now name the token; after the sync
TESTING.md's fixture guidance names the AgentProcess id where it said provider
generation.

2026-10-09 fixture repair outside the rename: `land_tests` read `via_agent`
from the first child of the repair Process; with AgentProcess rows in the same
table that child can be the provider. The query now selects `kind='lf'`.

2026-10-10 observed, not repaired: four `python/tests/test_checkout_refresh.py`
cases set `LF_HOME`, yet `lf wt` verifies worktree ownership against
`~/.lf/loopflow.db` and fails on `tasks.planning_completed`, a column from main's
`local_planning` draft. It refuses before changing anything; the installed
schema is unchanged. The read of the account Home predates this Task.
`test_architecture` reports the same three map errors on LOO-443's commit.

Signal worth sponsoring: a CI check that runs the Task's `rg` over the tree
would keep the retired words from returning; it costs one line in lint.
