# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved compatible-writer
mechanism, rejected alternatives and live diagnosis. No product decision is
pending and no relaxation of preservation, trust or storage authority is assumed.

2026-10-27: The latest Task definition authorizes the telemetry archive bridge
after checkpointing; checkpointing alone does not restore installed19 sync.
This compress step's allocated destination is
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261027-compress-8FQuIfqc/tmp`.
The final archive operation follows validation and the explicit-path checkpoint;
its command result records whether the move succeeded. Earlier archives remain
untouched. This recovery changes neither the unresolved writer proof nor
Infrastructure's ownership of installation and live acceptance.
The prior kickoff destination remains recorded at
`0288f9d611755301b85bd36316a7b310ed8d9d3a:scratch/questions.md`.
