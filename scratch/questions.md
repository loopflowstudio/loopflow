# Integration decisions

Jack Heart authorized sustained iOS improvement after the two initial slices.
The following notes preserve their original boundaries; current work is in mobile-ios.md.

# LOO-718 remaining decisions

November 3, 2026: Jack Heart authorized native fixture implementation before
live connectivity. Reversible choices: SwiftTerm 1.20.0 for the native input and
accessibility proof; separate ios.yml including the shared project to avoid
parallel Desktop edits; observe first, then grant fixture control explicitly.
This single-client peer proves no contention policy and selects no remote takeover gesture.

LOO-593's codec and fixtures are consumed unchanged at their canonical paths.
Its current server allowlist does not expose WorkViewPlan/primarySession; the
new Session-to-owner lookup and fixtures have been consumed. The fixture can decode the existing shape but cannot claim
that live read works. No competing endpoint or owner resolver was invented.

Permission/Stop state lacks validated action bindings, so controls stay absent;
ordinary native terminal typing answers the harness prompt. Physical keyboard,
dictation/IME choice UI and VoiceOver quality need demo/device review. Remote
carrier/pairing, canonical observer geometry and hosting tier remain explicit
integration decisions. Full historical research is in commit 9e4587bb53fc4102e8682a3bba61e9776a6de5c2.

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
