# LOO-953 quotes and open choices

Copied from LOO-936 (`git show f80183a113:scratch/questions.md`), only what
bears on Process removal. Speaker: Jack Heart, 2026-11-16.

## What was said

- On per-kind Process tables: "do we need both?", "what are hte process tables
  for", "should that be proc.db then or something", "now that we've moved to
  having a machine server and processes dont even map 1:1 with agent sessions
  i am unconvinced something like lf top and lf ps even really makes sense".
- On "A Machine-wide view of what's running ... becomes a list of live
  Sessions and Graph runs from the server, not a Process tree": "Check".
- On bare `lf -b <skill>` runs the server does not hold: "These should
  probably register themselves in some db somewhere", then "maybe that is the
  process table all over again hmm", then "on the other hand can we just
  filter regular lf to ps?", then "go for it" (check it).
- On "rows are the source (pid and start time on the Session or Graph run),
  the OS is the check, no registry and no Process table, environment scan as
  a diagnostic": "all seems fine."
- On `outcome`: "i just wanted to flag to you as somethign that needs a better
  design"; "and like maybe lf processes and agent processes shouldnt be in the
  same table?"
- On attachment placement and Task state: "(1) on the session (2) scond"
  (attachment token and attached Process on `agent_sessions`; Graph and ship
  records carry their own end, result and error).
- On `lf history`: "what im most interested is being able to investigate
  things like tokens and time by wave or by skill or other things like that".
  LOO-936 builds that as `usage.db` turn rows.

## What the OS process list showed (2026-11-16 18:40 PST, macOS)

`lf ps --json` listed 219 Processes, all alive. `LF_STEER_SESSION` named the
Session on 46 of 46 `codex -c` and 23 of 24 `claude --name`. 21 showed no
`LF_*` variable: all 8 `lf holder` and 13 `lf` a person started. 278 OS
processes carried `LF_PROCESS_ID`, `LF_TRACE_ID` or `LF_HUMAN_SESSION`; 128 of
them were not in `lf ps` (inherited children: `browse` 50, python 24,
`codex-code-mode-host` 15, `claude` 10, `codex` 10, `uv` 4). Read from an
agent's Bash with no sandbox; a sandboxed read is untested.

## Open choices (headless calls; reversible)

See the plan's "Decisions" for the calls made at kickoff.
