# Session cutover compression review

2026-09-26 · LOO-298 · Reviewed `ca1be1116d` and the supplied working note.

No executable reduction selected. The preceding implementation attempt withdrew
its executable edits; Rust and Swift are identical to `eeb609881`. The required
Session cutover remains unfinished. Removing its live file readers before
converting their writers and importing history would lose conversations.

| Fact | Current owner | Required reduction |
| --- | --- | --- |
| Task review identity, title, feedback and current Run | `sessions`, membership through `runs.session_id` | Generalize this owner to all conversation kinds; retain expected-current-Run comparisons |
| Task capture, cursor and selected attempt | `flow_invocations`; shared Run constructor/decoder | Retain invocation/version/claim fences and historical cursor decoding; move taskless execution here |
| Interactive conversation | Manifest scan, name and resolution sidecars | Convert `lf/commands/run.rs::begin_run_capture` and lifecycle writers before switching inventory |
| Ask request, keyed answer and replacement | `human_session.rs::{read_ask_record,write_ask_record}` | Import completed answers and move replacement to Session; delete title copying and feedback reset |
| Taskless capture and review | `flow_run.rs::{read,write,update}`, `flow_session.rs` | Import capture/cursor and review identity; delete `position.json` authority and standalone lookup |
| Session ancestry on the wire | `SessionRecord.work/wave_id/work_path`; Swift grouping | Move Rust, Swift and fixtures together to typed current-Run parents; retain bound Tasks absent from the roadmap |

Inspected the constructor, Session transactions, current-attempt selection,
forward constraints, launch recording, list/lookup/action dispatch, replacement
callers, public Rust/Swift fields, workspace grouping and the documentation's
prepared-Run paragraph. The restored slice review names each reachable old
reader and its replacement dependency. These are implementation dependencies,
not an external blocker or a request for another product decision.

Keep Session and Invocation current pointers: the former retains conversational
identity after closure; the latter fences the attempt acting at the cursor.
Keep exact process exclusion and immutable publication evidence. Keep historical
Started evidence until general Run recording/import covers its callers. No
unused adapter or representation was found whose deletion completes an ownership
boundary in the current code. Renaming or moving these paths would not do so.

Review evidence was missing from the checkout: `021d24f7b` preserved
`concept-review.md`, then `ca1be1116d` (`lf pr open: prepare branch`) deleted it
and `data-model-slice-review.md`. Restored both byte-for-byte from their committed
versions. The supplied ledger's claim that `ca1be1116d` preserved the concept
review is inaccurate; its earlier checkpoint is the preservation source.

Measurement reran the retained `review-lines.py` method: against `4cd64be3d`,
**+1,617 / −556** across 18 production Rust/Swift files, **zero deleted code
paths**. `human_session.rs` has **2,350** lines before the test attribute (2,351
including it). This pass adds/removes **zero executable lines**. The method
excludes test files and trailing test modules, retaining inline test helpers;
these numbers do not replace Jack's whole-file measurement.

No behavioral test reran: executable bytes are unchanged. Previous proof retains
the restored slice review's limits; it does not prove all-kind SQL inventory,
filesystem import or installed acceptance. Restore-content comparisons and
working-diff whitespace pass. No publication, Home mutation or Flow navigation
was performed. The complete read-path cutover remains the next implementation.
