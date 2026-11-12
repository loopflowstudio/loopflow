# Simplification Opportunities

Opportunities 1–4 (a host is a TerminalId, relayed launches held, viewing on
`held_terminals`, one model in the docs) are done on this branch; the analysis
is in git (`git show 64f2b3697f:scratch/simplification-opportunities.md`).

## Not this Task

The control model (one viewer holds an input token, **Move here**, Busy,
Ctrl+] detach, the `LF_SERVER_STATUS` handshake in `transport/connection.rs`)
was built for a conversation owned by one terminal. `server-first.md` plans
"every view types; size follows the newest view". It is the largest remaining
cascade and a product change people will feel, so it wants its own Task and
demo. Swift's `OwnerProtocol`/`OwnerStream` names go with it.
