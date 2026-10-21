# LOO-559 assumptions (2026-10-21, kickoff, headless)

- A macOS process with `P_WEXIT` set counts as exited, like a zombie: it can no
  longer run user code. Reversible: drop the `X` mapping.
- SIGKILL wait grows from 2s to 10s with a final observation. It only costs
  time when a member truly survives. Reversible constant.
- Survivors are named by `p_comm` and state, never full arguments.
