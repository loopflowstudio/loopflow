---
asana_id: '1213877387454414'
linear_id: 02536e4b-af6f-4c18-bd6c-7db0e00cd4c6
notion_id: 32af8f99-3d81-81e7-b1f5-f8d867892fa6
---
# Wave discovery and root wave

**Finish line:** lfd discovers waves from `wave/` on disk, reconciles them against the store, and runs them. Concerto creates or repairs the root wave when the repo has member waves on disk.

## What to build

1. **Disk scanner.** On startup and periodically, scan `wave/` for YAML configs. Reconcile against waves in the store: create new, update changed, mark removed.
2. **Root wave auto-creation.** When Concerto launches (or on first `lfq` command), create the root wave if it does not exist. Its `area` includes the discovered active wave directories and its flow is `garden`.
3. **Owner filtering.** Eventually filter discovered waves by an `owner` field in YAML. Initially, run everything.
4. **Reconciliation.** Handle wave YAML added to disk, removed from disk, or changed on disk without destroying runtime history.

## Done when

- lfd discovers wave configs from `wave/` and creates waves in the store
- Root exists with correct membership after Concerto launch
- Adding a new `wave/<name>/<name>.yaml` to disk creates the wave on the next scan
- Removing a wave YAML marks it inactive without destroying history
