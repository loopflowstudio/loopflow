# LOO-636 assumptions

- Reclaimed bytes use APFS `ATTR_CMNEXT_PRIVATESIZE` (what deleting a file frees now) in
  `disk::allocated_bytes`, for every Home and worktree measurement, not only snapshots.
  A store clone otherwise reports ~10 GB "freed" when its removal frees only its diverged
  blocks. Side effect: the live database's footprint excludes blocks it shares with
  copies. Linux keeps `st_blocks`.
- Ownership proof is the existing `copies/.lock` that `store::copy::create` holds from
  staging to rename: a free lock means every `copies/.tmp*` belongs to a dead process,
  so the sweep ignores the 24 h grace. A brief fork-inherited flock only delays a sweep.

