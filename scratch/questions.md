# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No product decision or relaxation of
preservation, trust or storage authority is assumed.

Jack Heart authorized this Task-local LOO-664 sync bridge on 2026-10-27: validate
and checkpoint explicit paths, then atomically archive complete `.lf/tmp` bytes
and leave it absent for automatic sync. Checkpointing alone does not repair
installed19. Preserve new telemetry separately if recreated; never delete logs,
overwrite archives, change ignore rules, force staging or suppress Git errors.
LOO-664 / #1754 is integrated; its disposable reproduction proves only the bridge.
Infrastructure owns CLI/helper installation and installed acceptance.

Prior archive paths, receipts, unused reservations and their earlier history:
`aaa11b0433b9b096d54a50b8a65a170607497d67:scratch/questions.md`.
Archives remain untouched. A reserved destination proves no move; its sibling
`receipt.json` records completion, checkpoint and byte verification. Source absence
applies only at the recorded move; retain later telemetry separately.

The compress inspection still reports installed `lf 0.14.19`; integrated source
v0.15.0 proves no installation or credential recovery. `.lf/tmp` is absent;
archive any telemetry recreated before returning to automatic sync.
