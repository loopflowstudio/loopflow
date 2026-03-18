# 04: tmux Architecture Study

**Finish line:** understand which parts of tmux's server/client architecture should directly inform `lfd`'s daemon-hosted shell model, and which parts should stay tmux-specific.

## Why study tmux

tmux already solved the hard parts of long-lived terminal hosting:
- server/client split
- detached and reattached sessions
- stable identity for sessions, windows, and panes
- PTY ownership living on the server
- multiple clients attached to one live session

`lfd` should not clone tmux's product surface, but it should learn from the shape of the system.

## Questions to answer

1. **Server boundary.** How does tmux keep PTY ownership, process supervision, and client attachment in one daemon without pushing shell semantics into each client?

2. **Identity model.** Which tmux concepts map cleanly onto loopflow?
   - tmux session
   - tmux window
   - tmux pane
   - client attachment

3. **Detach/reattach semantics.** What state survives disconnect? What is live-only? What has stable IDs?

4. **Control protocol.** What can we learn from tmux control mode and socket protocol about keeping the transport structured instead of scraping terminal text?

5. **Multiplexing policy.** Where should loopflow borrow tmux's plurality — multiple attached clients, multiple live shells — and where should Concerto intentionally stay calmer and foreground one run?

6. **Remote shape.** Which tmux ideas make remote `lfd` shells safer and simpler: socket-like auth, attach permissions, scrollback ownership, reconnect behavior?

## What to produce

- A short architecture note under `scratch/` or `wave/lfd/` comparing:
  - tmux server/client/session/window/pane
  - `lfd` / run / terminal session / worktree / client attachment
- A clear list of concepts to copy, adapt, and avoid
- At least one recommendation for the first PTY transport that gets the tmux lessons without requiring tmux itself

## Done when

- We can explain the `lfd` shell model in tmux-like terms without confusing wave/run identity
- We know whether multiple attached clients, scrollback persistence, and pane-like composition belong in the first transport or later
- The PTY/session design for `lfd` is borrowing from tmux deliberately instead of accidentally reinventing it
