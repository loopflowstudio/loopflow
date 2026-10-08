# Remote Task adoption demo — 2026-10-08

Superseded scope: Jack Heart subsequently selected the host callback in
[this Task’s design](work-on-another-machine-name.md). These results remain
evidence of the earlier adoption path, not acceptance of callbacks.

Built and exercised LOO-412 at `0cd8e7f14a6bffecd9323541e7689570316b892a`.
[PR walkthrough](pr-review.html) follows the public command through transport,
Task adoption, identity and preservation. [Current design](work-on-another-machine-name.md)
owns the scope. Published PR #1491's head matches this revision; its base is
`35e759aaf4e79e7dc8eef2e8d30048f10172b45a`.

## Direction and correction

Jack Heart said “have to demo with a cli you build from this worktree,” then
requested `pr-review`. That supersedes the earlier demonstration approach:
installed `/Users/jack/.local/bin/lf` is 0.13.9 and cannot resolve `machine`,
but that is not a prerequisite for reviewing this branch. No experiential
acceptance, design change, installation or landing was authorized in this exchange.

## Captured behavior

Both sides ran this checkout's `target/debug/lf`. The public selector invoked
`--machine fixture --task INF-123 --isolate --batch skill inspect-code`, then
repeated by machine ID. Isolated stores, a local bare Git remote and separate
CLI processes were real. SSH executed through a local transport adapter;
GitHub and the agent were simulated. No live credentials or installed store
were used. [Full output](remote-task-demo-output.txt) retains commands and errors.

| Scenario | Observation |
| --- | --- |
| Absent target Task | Target began with zero Tasks; its clone predated the branch. The skill read `already implemented` from the pushed code. |
| Repeat by issue name | Two launches read that implementation from the same checkout; one Task, one PR record and one Task worktree remained. |
| Two fresh stores | Both adopted `task_ceba43aa1cd754c49b273f79a40718a6`; the planning timestamp remained `1791360000`. This scenario supplied Task source directly to the recipient CLI. |
| Target behind a newly pushed commit | The public command exited 1 naming the branch, required commit and `lf sync`; prior HEAD and `local-notes` survived. Recovery itself was not run. |
| Missing source code | Direct transport-operation checks named uncommitted work and an unpushed commit, preserving source bytes and its legacy ID. Missing remote branch/commit checks created no Task. |
| Removed target issue | Copied planning did not revive it. |

The capture copied the existing five tests to a temporary integration target,
added observations and used the issue name for the public launches. Product and
published test sources stayed unchanged. The temporary target was removed.
[Capture script](capture-remote-task-demo.py) and [capture diff](remote-task-demo-capture.patch)
make the adaptation reviewable.

Checks: `cargo build -p loopflow --bin lf` and `uv run --no-sync python scratch/capture-remote-task-demo.py` pass; five scenarios passed in 13.39 s under external-network isolation. Full gate remains deferred to gate.

Built CLI SHA-256: `48241be36c6655b8d8123b64ea14bcae1e03d4fe3b2637487ceed7bddb56f8f1`.

## Review finding and limits

Every connection warned that versions differed despite both sides using the same
binary. Unchanged `machine::report_version` compares local `CARGO_PKG_VERSION`
(`0.13.9`) with a remote revision/dirty suffix (`0.13.9+0cd8e7f14.dirty`). This is
misleading source-build feedback inherited from main; no repair was selected.
The missing Linear credential warning retained confirmed direction in the fixture.

Real SSH/authentication and installed behavior remain unobserved. Neither two
pre-existing legacy IDs nor populated target Workflow/Session history, existing
Project selection, pending rotation or separate invalidation preservation were
exercised. These are the design's remaining gate checks; the passing capture
cannot stand in for them or for Jack Heart's review.

Recommended next action: review [scratch/pr-review.html](pr-review.html), then
finish the identified preservation proofs in gate. Real-machine acceptance can
use source-built CLIs in isolated environments; release installation is not a
prerequisite. No Session was closed and no Flow restarted.

The HTML was visually inspected at 1440 and 390 pixels. Separate section captures
checked code wrapping and an expanded evidence disclosure. Headless fragment-URL
captures returned blank frames, so section extracts supplied that visual check;
anchor destinations were checked statically. Supporting screenshots are
`pr-review.png`, `pr-review-narrow.png`, `pr-review-code.png`,
`pr-review-code-narrow.png` and `pr-review-evidence.png`.
