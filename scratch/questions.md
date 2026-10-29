# Questions

- Repeated-failure stop (2026-10-29): a single `lf task run` keeps its 3 attempts,
  because `a_stopped_run_is_started_again_from_where_it_began` shows a cause that
  clears between attempts. The run reports a repeated error only when every attempt
  shared it, and `lf ship` then withdraws Auto-Ship rather than restarting. A long
  network outage that produces the same error 3 times in a row also stops shipping.
  `lf ship` resumes it. Revisit if that proves too eager.
