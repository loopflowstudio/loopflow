# LOO-575 open questions

- **Command shape (assumption, 2026-10-24 kickoff).** The Task left the choice
  between a detach flag and making headless `lf task run` detach by default.
  Chosen: bare `lf task run` returns once its Graph is running; `-b` keeps
  "return when the work ends"; `-i` stays attached. This changes what a person
  sees typing bare `lf task run` at a terminal (placement output, then the
  prompt back, instead of streaming the Graph). Reversible: a flag would invert
  the default with the same mechanism. Review at demo.

- **Single-skill edges (assumption, 2026-10-24 implement).** An edge naming a
  skill records no GraphProcess, so a bare run lets go once that skill's Process
  registers and prints `running as Process <id>`; its log is
  `graph-<process id>.log` though no `Graph:` line names it in status. Simplest
  safe choice; revisit if skill edges need their own status line.
- **Retry output (implement).** After letting go, the inner run's own output
  (retry notices, arrival errors) stays in the first attempt's log; later
  attempts' Graph children write to their own `graph-<id>.log`.
