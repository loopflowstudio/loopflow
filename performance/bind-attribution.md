# Usage attribution after a bind

```bash
lf usage --binds          # bound Sessions, then Task and Wave totals under each rule
lf usage --binds --json   # the same comparison, every step counted
```

Jack Heart requested this evaluation on 2026-09-30 (LOO-336) after keeping
prospective attribution for LOO-298. The report reads immutable Session history
and changes nothing: it shows what each rule would say.

| Column | What it counts |
|---|---|
| `BEFORE` | Steps that finished before the bind under another owner |
| `DURING` | Steps running or unfinished at the bind; prospective cannot split them |
| `AFTER` | Steps recorded to the Task itself |
| `PROSPECTIVE` | An owner's total as recorded when the usage was spent |
| `POST-HOC` | That total plus `BEFORE` and `DURING` of its bound Sessions |

A step is one captured Session input. `*` marks steps with no reported usage.
Only Tasks and Waves whose totals differ appear.

## Reading on 2026-10-01

No bind has happened on Jack's machine. The installed `lf` 0.12.28 has no
`lf session bind`, and its store has no `agent_sessions` or `session_events`
tables, so there is no bound Session to measure and no Task cost moves under
either rule. The frequency and cost questions stay unanswered until a release
carrying Session history has been in use; rerun the report then.

## What post-hoc would change

- **Task totals** (`lf usage --task`, the `--weekly` Task breakdown, `lf history show`,
  `lf history`): a Task's past weeks grow on the day a Session is bound.
- **Wave totals**: grow only when the bound Session had no Wave. Bind keeps an
  existing Wave, so usage never leaves one Wave for another.
- **Published Wave readings** (`context-*`): unchanged. They are machine-wide
  ratios with no owner in them.
- **Budgets**: unchanged. The only budgets are prompt context budgets; nothing
  enforces spend from attributed usage.
- **Taskless usage**: shrinks by the same amount, after the fact.

Post-hoc removes the `DURING` problem: the whole Session belongs to the Task, so
no turn needs splitting.

## Recommendation

Keep prospective until `lf usage --binds` shows real binds. Switch to post-hoc
if `BEFORE` plus `DURING` is a material share of bound Tasks' totals: the
readers that would move are all reports, none is a budget or a published
reading, and the unknown split disappears. If the share is small, the choice
does not matter and prospective stays as the smaller system. The decision is
Jack's.

The switch is one change in the Session history reader: a bound Session's
inputs take the Session's Task and Wave in place of the owner recorded on each
event. Stored events stay as written.
