# LOO-615 gate — 2026-10-27

Gate is incomplete; no current-tree pass is claimed. Started from clean
`b23853c065`, comparing PR #1718 against its `main` base. During review, another
process began merging `b84940a7da79bbd40b79be31beb015fc40804375` into this checkout.
Its edits and merge ownership were preserved.

Review found no additional defect in the original Process-retention diff.
The shared predicate covers the current lf Process reference inventory, including
Task shipping and Session transport. The latest exit writer commits its upsert
and two deletes separately; interrupted cleanup is left for prune. Review the
completed merge before accepting this conclusion for the final tree.

Checks:
- `uv run python scripts/test.py --reuse-passing` — resource preflight failed: 20.3 GiB free versus the required 32 GiB reserve, even after its supported recovery; affected architecture/Rust/website/Swift suites did not start. Defer to capable CI or rerun after capacity is restored.
- `uv run pytest python/tests/test_third_party.py` — 11 passed, 2 failed while the concurrent merge had conflict markers in `inventory.json`; this is invalid final-tree evidence. Rerun after merge resolution and inventory refresh.
- `git diff --check` and `git diff --cached --check` — passed after the concurrent worker removed conflict markers; merge was still in progress.
- Quiet base/branch `store_contention` comparison — deferred: host load exceeded 100 and disk preflight failed. Earlier contention failures remain unresolved by this gate; do not claim a latency or reliability improvement from splitting statements without that comparison.

Historical acceptance evidence, recovered from the notes removed by
`5cac39469d` (not rerun here): the clone probe retained 104,998 of 953,191 rows,
had no foreign-key violations, and shrank 10.5 GB to 287 MB; 507 finished
children had running parents. Twenty outside-checkout Task diff polls emitted
no changed Task/Wave frames; activity frames reflected the live processes.
These measurements predate the latest exit-write change and current merge.

Delivery copy needs reconciliation: PR #1718 still describes an immediate
transaction around `record_process`; the current writer uses separate statements.
Its claim that the Work-watch check is still owed is stale (evidence above).
Keep the quiet contention comparison and display-only walkthrough unproven.
The complete merged tree still needs formatting, Clippy, affected suites,
released-frontier migration coverage, headless Desktop tests/build and current
third-party inventory validation. No process started by this gate remains running.
