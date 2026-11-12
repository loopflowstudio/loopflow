# Open choices (LOO-863 QA pass)

- **The move carries the laptop's planning key** (`--planning-key` on the check)
  and a Machine with none adopts it. Taken because a cold acquire otherwise mints
  mini a key forever. The alternative is to refuse and print
  `lf repo plan key --recover <key>`; it costs a step on every new Machine.
- **An unreadable PR no longer fails a Task checkout** (`prepare_placement`):
  it is logged and left unknown. A GitHub-only guard broke `two_people`, whose
  fake GitHub sits behind a local origin. If a checkout must know its PR,
  scope the tolerance to arrivals instead.
