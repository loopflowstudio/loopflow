# Open choices (made for review)

- **Wake `agy` by typing into its held terminal.** The alternative is a
  provider inbox, and agy 1.5.16 has none an outside process can reach (see
  plan). The steer appears in the TUI as a pasted prompt, not as a `[Message]`
  from a sender. If agy later loads workspace sidecars or gives hooks
  `ANTIGRAVITY_CSRF_TOKEN`, an inbox through `agentapi send-message` would
  replace this.
- **Draft guard is "no view input since the conversation yielded"**, not
  reading the prompt line off the screen. A person who types and then clears
  their draft goes without a wake until their next Enter.
- **Viewer handoff (LOO-1097) and the unanswered question (48edb9)** go into
  the parity table as typed gaps with those owners. This Task doesn't fix them.

# Probe side effect (2026-11-24)

While probing, two `agentapi send-message` calls from this checkout's shell
used `ANTIGRAVITY_*` values inherited from conversation
`c3742012-2e46-47c2-a718-aa39e90fe213` (agy pid 49362, the one that ran
`lf task run LOO-1098`). Both messages went to throwaway probe conversations
in `/tmp/agyprobe`, but they were sent through 49362's server and attributed
to `c3742012`. That conversation may show those two sends.
