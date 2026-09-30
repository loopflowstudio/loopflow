# Open naming proposals

LOO-298 · Jack Heart · 2026-09-30.

| Current table | Proposal | Decision |
| --- | --- | --- |
| `session_events` | `agent_events` | Jack said “maybe”; not selected or applied. |
| Deleted `run_events` journal | `exec_events` | Jack explicitly kept the proposal open on 2026-09-30; no rename selected or applied. |
| `flow_events` | Keep | Names FlowSession history. |

The old `run_events` journal was deleted during compression. There is currently
no table to rename to `exec_events`; Exec command results live on `execs`.
This observation does not close Jack's proposal or authorize a replacement
event table. Neither open proposal blocks the single-PR landing.

Process APIs use Exec; captures and conversation history use Session; Flow
compilation uses `compile_*`. Definition provenance remains `sources` because
Desktop displays its breadcrumb. Current captures use current field names;
no legacy codec or naming alias is required.
