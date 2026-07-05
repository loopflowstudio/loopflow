# M2 Implementation Notes

- Asana roadmap lookup is blocked locally: `lf op pm show --wave goals` reports an expired stored Asana token. Proceeded from `scratch/m2-substrate.md` and `wave/goals/MEMORY.md`.
- The loopflow guidance says to dispatch with `lfq worker run`, but this checkout's `lfq` has no `worker` command. Proceeded inline rather than falling back to the public `lf q worker run` API M2 is supposed to remove.
- This pass removed the public container/mode service surface but did not delete the deeper `lfdb` postgres backend or `StorageConfig::Postgres` dispatch arms. That backend is still the next substrate cut.
