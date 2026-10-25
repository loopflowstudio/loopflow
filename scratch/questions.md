# LOO-575 open questions

- **Command shape (assumption, 2026-10-24 kickoff).** The Task left the choice
  between a detach flag and making headless `lf task run` detach by default.
  Chosen: bare `lf task run` returns once its Graph is running; `-b` keeps
  "return when the work ends"; `-i` stays attached. This changes what a person
  sees typing bare `lf task run` at a terminal (placement output, then the
  prompt back, instead of streaming the Graph). Reversible: a flag would invert
  the default with the same mechanism. Review at demo.
