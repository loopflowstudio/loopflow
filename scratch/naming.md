# Remaining naming proposal

LOO-298 · Jack Heart · 2026-09-30.

| Current table | Proposal | Decision |
| --- | --- | --- |
| `session_events` | `agent_events` | Jack said “maybe”; not selected or applied. |
| `flow_events` | Keep | Names FlowSession history. |

The old `run_events` journal was deleted during compression, so the earlier
`exec_events` rename proposal has no remaining table to rename. Exec command
results live on `execs`.

Process APIs use Exec; captures and conversation history use Session; Flow
compilation uses `compile_*`. Definition provenance remains `sources` because
Desktop displays its breadcrumb. Current captures use current field names;
no legacy codec or naming alias is required.
