# Task codes: `ABC-123` with or without a tracker (draft, 2026-10-19)

Jack Heart wants every Task to carry a Linear-style code — `LOO-124` — with the
experience without any PM tracker as the priority, and Linear, Asana or Notion
able to drive the same system. One planning provider per repository (already true:
`planning:` is one value).

## Today

- A local Task's code is the shortest unique hex prefix of its ID (6+ chars,
  `short_task_id`). It can differ between machines that know different Tasks, and
  it names branches: `jack/3fa9c1-fix-auth-flow`.
- Linear replaces it with Linear's `LOO-520` once the export is accepted.
- No repository counter exists. Git planning is a field-level last-writer-wins
  document (`planning_exchange.rs`) that carries `task_identifier` but nothing
  enforces uniqueness.

## Sketch

- **The code comes from one allocator per repository.** Linear: its team key and
  number. Notion: its `unique_id` property (Loopflow adds one with the repo prefix).
  Git and Asana: Loopflow allocates `PREFIX-(max+1)` over the repository's known
  Tasks at creation.
- **Instant, then confirmed.** `lf task create` fetches, allocates, saves and
  publishes in one command, so online creation prints a confirmed code.
  Offline, the code is provisional until the next sync.
- **Deterministic repair.** When a merge brings two Tasks with one code, the
  earlier `(creation clock, Task ID)` keeps it; the other takes the next free
  number. Every machine computes the same answer from the same document.
- **Old codes stay valid.** Each Task keeps every code it has had; the selector
  resolver accepts them, and branches already named after a code keep working.
- **Prefix:** Linear's team key; otherwise `planning.prefix`, defaulting to the
  repository name's first letters (`loopflow` → `LOO`).
- **Mirrors:** Notion shows its own property; Asana gets a description footer, or
  its ID custom field when the workspace has one (paid plans only).

## Driving from a tracker (audit before building)

| | Linear | Asana | Notion |
|---|---|---|---|
| Created there → appears in `lf task list` | yes | no (acquisition missing) | no (acquisition missing) |
| Title/description edit → Task updated + steer | yes | yes | yes |
| Comment → steer | yes | yes | needs comment capability |
| Status change there → local state | yes | yes | yes (stand-in) |

## Open

- Where this lands: this Task, or its own Task stacked on LOO-520.
- Asana allocation race (two machines in one instant) resolves by repair, which can
  renumber a code someone already saw; the alias keeps it resolving.
