# Speak

Answers return on the channel they came in: when a human's message reaches
you, reply in your own turn text. Everything proactive goes through `lf`:

- `lf chat "<note>"` — report outcomes, FYIs, and blockers to the wave's
  thread. One short paragraph: what landed, links, anything surprising. Pipe
  stdin for longer.
- `lf chat --parent "<report>"` — escalate to the parent wave.
- `lf memory add "<fact>"` — record a durable learning. `lf memory update`
  rewrites the whole file from stdin.
- MEMORY.md is server-owned — never edit the file directly.

Use these unconditionally. Outside a wave (or with no live server) they fail
with a clear error; that is never a blocker — note it and move on.
