# 03: Daemon-Hosted Shells

**Finish line:** `lfd` owns attachable shells / PTYs in fresh or existing worktrees, and Concerto or SSH-style clients attach to those sessions to run normal `lf` commands by hand.

## Context

Interactive execution should not be a launch-spec shim wrapped around a raw agent command. The real interactive model is a daemon-hosted shell: `lfd` owns the session, clients attach to it, and the terminal shows the same `lf design` or `lf build` command a human would type anywhere else.

This needs to work locally first, but it should be shaped like the long-term remote model. "SSH-like" is the product goal: attach to a real shell in the right worktree, detach, reattach, and keep the run/session relationship intact.

## What to build

1. **PTY/session manager.** Create, attach, read, write, resize, detach, reattach, and close daemon-owned terminal sessions.

2. **Worktree selection.** Support both:
   - fresh shell in a new or prepared worktree
   - attach to an existing shell tied to a live run or workspace

3. **Session identity.** Make terminal sessions first-class records with stable IDs, run/worktree association, lifecycle status, and reconnect-friendly metadata.

4. **Client transport.** Replace launch specs with an attach protocol. HTTP plus websocket streaming is fine if it keeps `lfd` as the process owner.

5. **SSH-style path.** Shape the interface so a future remote client can "ssh into lfd" in product terms even if the first implementation is not literal OpenSSH.

6. **Exit-driven reconciliation.** Session exit should resume, fail, or otherwise advance the relevant run through daemon-owned process observation rather than a shell callback.

## Open questions

- How much scrollback should survive detach and reconnect during one daemon lifetime?
- What auth model should remote or cross-machine attach use?
- When multiple runs exist for a wave, what should the default attach target be for product clients like Concerto?

## Done when

- `lfd` owns the PTY lifecycle for interactive sessions
- Concerto can attach to a daemon-owned shell without launching a local wrapper command
- Interactive terminals show normal `lf <flow-or-step>` commands
- Detach/reattach works without losing run/worktree/session association
- The same primitive can later back SSH-style access without inventing a second interactive model
