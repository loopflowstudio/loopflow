# LOO-728 recovery — November 4

Managed recovery is complete. The extracted implementation is checkpointed at d1136c3e3f3ee6536b3de7ddba308ab0dfca0793; owned sync produced 2d5126b8ad335cccd4674daa846ae5d34b90d95d. Task placement now records main a5000ef889c611bf41b563b8ff40db10ed29d4bf, with no stack parent, and GitHub PR #1817 targets main. These commits remain local for the primary conversation's delivery steps. Source/parent work and whole-work checkpoint ea0e0781b22f092b6a0169802997ff7a3628c65c remain intact.

The missing base 3bdaad5c6f7c301e2e45ca8c6184097377c5d7e6 had blocked three implementation launches. Git verified df3b5e810d3841a19bc5911c332fb42f23193d46 as HEAD's common ancestor with the recorded parent's remote branch. Managed sync now uses that ancestry when its recorded fork is absent, without mutating placement on reads. The cause of the missing base remains unproven; earlier temporary-copy tests are not evidence of this checkout's behavior.

Machine boundary: the rebuilt main-based target/debug/lf cannot read the installed schema (machines.is_local is absent). The retained, tested pre-sync development executable target/debug/deps/lf-479145c5a7dd7445 completed `sync --continue` through the ordinary APIs. No direct SQLite writes, installation, owner restart or Graph launch occurred. This incompatibility belongs to the installed/main schema boundary, not a reason to restore parent changes to this PR.

Startup evidence: installed `lf --task LOO-728 --context -b skill implement` exited 0 and emitted 38,915 bytes with the repaired main base and no stack parent. Normal context assembly is restored; the primary conversation owns resuming delivery. Attached Desktop rendering remains for demo/review; focused validation is recorded in write-docs-first.md.
