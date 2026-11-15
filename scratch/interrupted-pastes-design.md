# Interrupted terminal pastes

A bracketed paste starts with `ESC [ 200 ~` and ends with `ESC [ 201 ~`.
The provider remains alive across viewer loss and server restarts, but currently
nothing finishes a paste whose sending connection disappears. The native
parser then treats later keys and Returns as paste content.

Keep one small streaming delimiter tracker shared by hosted input and holder
writers. Pass every byte through immediately; track delimiters across read and
write boundaries without retaining paste content. When input control leaves or
changes, append only the unfinished paste's closing delimiter (or its missing
suffix). The holder does the same when its server writer disconnects, covering
server restart. Never append Return, replay content, or impose a paste timeout.
Ordinary typing and complete pastes stay byte-for-byte unchanged.

Verify disconnected/replaced viewers and a lost holder writer against an actual
raw PTY. Check split markers, complete pastes and partial writes in focused unit
tests. The exact transition that lost LOO-904's marker remains unknown.
