# Stack parent: LOO-593 (PR #1813)

This Task stacks on LOO-593's branch at `67a968bf84`. The parent's design notes
were removed from this checkout to keep context within budget; its land runs
`lf scratch clear`, which removes them from main too. Read them at the parent
commit, not here:

| Note | Reference |
| --- | --- |
| Accepted local architecture, FFI deletion, LOO-718 handoff, remaining proof | `67a968bf84:scratch/mobile-backend.md` |
| Rex comparison that motivated revision 2 | `67a968bf84:scratch/rex-server-comparison.md` |
| Jack Heart's direct-write direction and acceptance | `67a968bf84:scratch/server-write-direction.md` |
| Direct-write native demo; earlier cutover demo and handoff | `67a968bf84:scratch/direct-write-demo.md`, `…/server-cutover-demo.md`, `…/server-cutover-handoff.md` |
| Parent decisions and retry history | `67a968bf84:scratch/questions.md` |
| Original Desktop server scope (October 25) | `67a968bf84:scratch/research_notes/desktop-server-original-scope.md` |

Live facts this Task relies on: Jack Heart accepted direct server writes on
November 4, 2026 and authorized publication and interactive demo of PR #1813
without merge, Auto-Ship or Task completion; LOO-718 owns remote admission,
pairing and iOS in `/Users/jack/src/loopflow.mobile`, which this Task does not
edit. [server-protocol-2.md](server-protocol-2.md) is the parent's accepted
revision 2 design; [server-performance/results.md](server-performance/results.md)
holds the direct-write baseline this Task measures against.
