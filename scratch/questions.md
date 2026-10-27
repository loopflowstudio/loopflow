# Questions / assumptions (024952)
- Revert PRs are opened with GITHUB_TOKEN, so their own CI does not start until someone pushes or reopens them. The PR body implies this. Chose this over a PAT: it needs no new secrets.
- Bisect runs use main's current workflow definitions on older commits.
- `Cargo.lock`/`Cargo.toml` bumps no longer run macOS pre-merge (not in the accepted trigger sets); post-merge catches macOS-only breakage.
- `scripts/test_network.py` (shared by every job) is in swift-test's trigger set because swift-test runs it with `--desktop`.
- Land resync also merges main when GitHub reports BEHIND on a repo without a merge queue; otherwise strict required checks would leave auto-merge waiting forever. Loopflow itself has a queue, so this never fires here.
- The website's `lf-reference` command tree comes from whichever `lf` the deploy host runs (`LF_BIN` or PATH). The release publisher deploys after promotion, so that is the released CLI; GitHub shows only the preamble.
- One resync rule: a queued PR never needs main merged, even when GitHub reports DIRTY (the queue evicts it instead); `lf land` used to merge main there. The release wait now also rebuilds a release PR the queue evicted for `merge_conflict`, where it previously ran a CI repair.
