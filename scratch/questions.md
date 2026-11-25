# Settled direction & open notes

- **No parity table or `Support::Gap` (Jack Heart, 2026-11-24):**
  - `"i would rather make it harder to add a harnes if it makes it harder to miss imortant behavior. when i add a harness, i want it to have all the good stuff"`
  - `"1. Just remove the table. Instead, just make sure that all behaviors are asserted to work and testeed  across all harnesses."`
  - `"and that the Harness interface demands full functionality"`
  Remove `harness/parity.rs` and the `harness/README.md` parity table; put required mechanisms directly on `Harness` / `Launcher` / `Capabilities` and assert every behavior unconditionally across `HarnessKind::ALL`, isolating only the pre-existing gaps (LOO-1099 live `agy` idle wake, Task 48edb9 headless questions) as narrow inline exceptions with no wildcards.
- **Reopen on Antigravity:** Use Antigravity's native interactive prompt (`-i "Read the waiting steers and Task updates for this Session."`) to fire `PreInvocation`, keeping steers unread until the hook prints them. Open UX detail noted in QA: that generic opening line is visible as the first turn in the TUI.
- **Already-open idle Antigravity wake:** Owned by LOO-1099 (live PTY probes ruled out `?2004h` paste-mode readiness due to trust dialog, slash menu, model picker, and server-adoption draft safety).
- **Viewer handoff:** Owned by LOO-1097 (live provider TUI behavior).

- `AI_AGENT` stays in the shared markers. A nested launch clears it regardless of which provider set it.

# Probe side effect (2026-11-24)

While probing, two `agentapi send-message` calls from this checkout's shell
used `ANTIGRAVITY_*` values inherited from conversation
`c3742012-2e46-47c2-a718-aa39e90fe213` (agy pid 49362, the one that ran
`lf task run LOO-1098`). Both messages went to throwaway probe conversations
in `/tmp/agyprobe`, but they were sent through 49362's server and attributed
to `c3742012`. That conversation may show those two sends.

# QA pass 2 (2026-11-25)

- Same steer, different outcome: on the `task-session` primary, Codex (steer as
  its own turn) answered the steer; Claude (cross-session message) reported it
  and waited "when you ask here", on both idle wake and reopen. Delivery is the
  same; behavior differs. Likely skill/primary-session framing, outside this
  diff. See scratch/qa.html.
