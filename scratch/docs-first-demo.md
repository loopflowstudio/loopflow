# LOO-728 — docs-first demonstration, November 4, 2026

Decision: advance to gate. This is the headless reviewer's decision under Jack
Heart's instruction to stand in at demo, not a new statement of acceptance by
Jack Heart. No product design changes are proposed. The configured Task
walkthrough failed before presentation; attached Desktop behavior remains
unobserved. The independent candidate's headless behavior passes.

Design: [docs first on main](write-docs-first.md). Recovery and the known
installed-schema boundary: [recovery](recovery.md). Jack Heart accepted
Desktop-only automatic presentation and independent-main delivery on November 4.
No person participated in this demonstration, so there is no new human feedback.

## Progress since the preceding pass

The earlier demo returned to implementation because the Task was still stacked.
The owned detachment and integration are complete. Current checkout and GitHub
PR #1817 both name `60dd0e3f4141a87f0f556e373b189a76d2e703d7`; the PR is OPEN
against main. Its supplied comparison base is `c59cfcc63160b55c84ded559646dbc29883b9f5d`.
The latest checkpoint only changes scratch review records. Source and parent work
remain preserved. This is new evidence of the requested progress, not a replay
of the original stacked walkthrough.

## Actual configured path — unsuccessful

After `cargo build -p loopflow --bin lf` succeeded, ran:

```sh
target/debug/lf open --host desktop --task LOO-728 \
  --file README.md --file docs/orchestration.md --json
target/debug/lf desktop state --json
```

The first command failed with `no such column: is_local` in
`SELECT id FROM machines WHERE is_local=1`, before file presentation. The second
succeeded and reported `windows: []` (plus an existing prototype opening).
Neither establishes attached Files rendering, saving through the mounted UI, or
conversation continuity. No app was installed, launched, or restarted for this
review. The failed configured command remains a failure.

This matches the previously recorded main-candidate/installed-database boundary.
It does not justify restoring excluded LOO-498 schema or transport work to this
independent PR. The origin of the installed-schema mismatch was not investigated
again in this demo.

## Current-candidate headless evidence

Using the rebuilt `target/debug/lf`, a fresh temporary `LF_HOME`, inherited
`LF_*` removed, and this actual checkout, ran the same file presentation without
`--task` (the temporary Machine has no Task registration). Initial presentation
and repeat both exited 0 and returned the absolute original paths for README.md
and docs/orchestration.md with `host:null` and `opening:false`. A missing file
`docs/loo728-demo-missing.md` exited 1 with `Cannot open … No such file or directory`.
The temporary Machine was removed afterward. These are isolated candidate
observations, not proof that the installed Task route works.

Both original documents had identical SHA-256 hashes before and after retry:

- README.md: `2b91aa628b8499c599d74aae16594aa30dfbbccd1b37104012121a7b7f044d35`
- docs/orchestration.md: `f8917531f29775785b51cab0baa549ac0a21ff5fa515a48016afe3a221f4f258`

Focused checks on the current candidate, with inherited `LF_*` unset and stdin
closed:

```sh
scripts/test_desktop.sh --filter 'TaskFilesTests|DesktopPaneControlTests'
cargo test -p loopflow --test integration \
  work_open_tests::desktop_file_presentation_without_desktop_never_falls_back_to_a_terminal
cargo test -p loopflow --lib graph::compiled::tests::pursue_loops_to_a_published_pr
```

Swift exited 0: its runner reported 54 tests in two suites, with four optional
display diagnostics explicitly skipped. Successful tests include ordinary-doc
presentation retaining an edited document and selection across repeat requests,
reusing one added tab, Markdown/HTML read mode, save reconciliation, and the
production Files action retaining workspace state. This is headless production
view/model evidence, never a screenshot or mounted UI observation.
The Rust fallback test passed across bare, Rex, Herdr, tmux and cmux contexts,
checking original saved content, zero Sessions, and no host launches. The Graph
test passed, covering write-docs first and loops back to implement. The library
test build emitted a linker unwind-size warning; no test failed.
Local command logs: `/tmp/loo-728-demo-swift.log` and
`/tmp/loo-728-demo-rust.log`; their substantive results are retained here because
those logs are temporary.

## Decision and remaining evidence

Advance because the preceding implementation's concrete detachment gap is
resolved, the accepted docs-first contract has fresh headless evidence, and no
new repairable defect within this Task's independent scope emerged. Jack Heart's
latest recovery direction explicitly says not to replay completed implementation.
[Swift testing](../swift/README.md#testing) and the
[optional hosted diagnostic policy](../release/UI_HOST_GATE.md) make mounted
native diagnostics optional for headless Task acceptance. Requiring an unavailable
display or reintroducing parent work would contradict those boundaries.

Gate still owns the combined candidate's affected suites, lint, and delivery
checks. This demo is not a full gate pass, proof of release, or approval inferred
from silence. No unresolved product choice requires Jack Heart's input.
Installed-database compatibility and the attached Desktop walkthrough remain
explicit evidence limits. A future configured walkthrough needs a compatible
candidate runtime/database and an already attached matching Desktop Files view;
it should check rendered originals, draft retention on repeat, save/readback,
and the same conversation. No new HTML page or screenshots were generated on
this headless surface. Existing screenshots belong to the earlier stacked tree.

Historical detailed evidence remains at
`ea0e0781b22f092b6a0169802997ff7a3628c65c:scratch/docs-first-demo.md`.
Earlier stacked-test passes and explicit Rex opening are not evidence for this
candidate and are superseded by the scoped account above.
