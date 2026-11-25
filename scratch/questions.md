# Open choices

- Use Antigravity's native interactive prompt to start a reopened work
  session's hook. Keep a steer unread until the hook prints it.
- Revisit live idle wake after finding a reliable prompt or provider address.
  The trust dialog and model picker observations in the plan rule out the
  proposed paste-mode readiness check.
- LOO-1099 owns the already-open Antigravity idle-wake gap.
- Check reopen at the provider boundary on the four server-delivered routes
  and through the native CLI launch on Antigravity.
- Keep viewer handoff as a real-provider check (LOO-1097) and unanswered
  questions as an automated gap (48edb9).

# Probe side effect (2026-11-24)

While probing, two `agentapi send-message` calls from this checkout's shell
used `ANTIGRAVITY_*` values inherited from conversation
`c3742012-2e46-47c2-a718-aa39e90fe213` (agy pid 49362, the one that ran
`lf task run LOO-1098`). Both messages went to throwaway probe conversations
in `/tmp/agyprobe`, but they were sent through 49362's server and attributed
to `c3742012`. That conversation may show those two sends.
