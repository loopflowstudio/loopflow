# LOO-593 decisions and gaps

Jack Heart authorized the installed local slice through demo, without Auto-Ship.
The simplest existing command is Session rename; no create/start or retry receipt
is introduced. Request IDs are correlation only and uncertain commands are never
automatically replayed. Owner observation/control semantics remain unchanged.

Full source research was checkpointed at `0bd889e4b860fd3d7c44389a86914317c0d22af6:scratch/` before curation.
The archived notes include contrary evidence: lfd already shared executors but
drifted in complete-operation/lifecycle ownership; setsid alone proves no
supervisor isolation; SMAppService updates may kill the helper; remote proxies
cannot borrow Unix peer trust. Exact remote transport, takeover UX, unattended
Mac hosting, create/start recovery and remaining Work cutover remain later work.

November 3 reconciliation: LOO-718's existing-Task → existing-Session path needs
`view_plan`; exposing the existing shared read is a bounded contract correction.
The additive contract was published at `12fbe9102c` on the Task branch. No remote
transport or takeover decision is inferred. Full installed-executable replacement with active Work and unchanged
provider-usage totals remain acceptance work; separate registrar/restart fixtures
do not establish them.

November 3 concept review: retain Desktop's deliberate retry policy for a lost
Machine connection. This is the simpler recovery interaction and preserves the
existing no-implicit-relaunch rule. The retry must reconnect the exact owner,
without invoking provider resume or taking a newer viewer's claims. This is a
derived implementation direction from Jack Heart's accepted continuity contract,
not a new approval of automatic takeover. Details and counterexamples live in
`scratch/mobile-backend.md`; no local product question needs an answer here.

November 3 implementation: keep the Machine relay alive and put deliberate
Ctrl+R reconnect / Ctrl+] close in its terminal, so connection loss never reaches
SessionsStore's ordinary preparation. Terminal recovery asks the exact owner for
a fresh screen because a local disconnect prompt changes the displayed screen;
nonterminal relays use retained Resume bytes and fall back to a fresh attachment
when the spool expires. Both retain the owner, reconnect token and byte position
in memory only, request free claims without replacement, and discard outage input.
Native presentation remains for the interactive demo.
