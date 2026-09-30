# v0.12.28

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.28 makes finishing work more predictable: Task lifecycle commands coordinate issue, PR, checkout, and branch cleanup while preserving delivery history and reporting incomplete cleanup. Contributor verification also fits unattended runs, with focused checks during implementation and broader acceptance at gate. Upgrade for fewer manual cleanup steps and Desktop checks that run without a display session or permission dialogs.

## Finish or cancel work with its cleanup attached

Task outcomes now own the cleanup that previously required separate commands. Completion protects unmerged local work, cancellation closes retained unmerged PRs, and retries preserve effects that already succeeded.

- Use `lf task abandon` with an issue ID, retained branch, or the current checkout to cancel the Linear issue and local Task, close retained unmerged PRs, and remove their checkouts and branches. Retries still work after the checkout disappears.
- Completed delivery is cleaned after its worker stops and settles its claim. Cleanup preserves dirty files and branch tips beyond the merged head. Standalone PR landing also cleans up; bare Task landing retains the checkout for continued work.
- `lf task delete` cancels unfinished placed work or cleans completed delivery before trashing the issue. Task, PR, and Run history remain readable.
- Preview old-chapter cleanup with `lf task sweep --json`, including eligible open issues in archived Projects. `--apply` rechecks membership before cancellation. Worker claims, open PRs, unreconciled merges, dirty checkouts, and unavailable evidence prevent cancellation.
- Live or unresolved workers block cancellation and deletion; pending cancellation prevents new worker claims. Cleanup failures report how to retry.

## Verify changes without a display session

Verification now belongs to the phase that can use its results. Implementation stays focused on changed code, while gate owns affected suites and automated acceptance; unavailable infrastructure routes checks to a capable environment without counting them as passed.

- Implement and compress run changed-code builds and focused tests. Gate runs broader checks once and reuses applicable results. Actual build and test failures still require repair.
- Default Desktop verification runs under WindowServer denial in gate and CI. It inspects production Work views in four states and invokes the real navigation button.
- Window, Metal, PTY, and hosted UI checks become optional diagnostics. The default gate drops the duplicate Desktop build and window-capture step.
- Headless inspection does not prove visual rendering or native integration correctness. Those remain part of opt-in diagnostics and demo/review.

## Operational notes

- **Command migration:** replace `lf wt remove` and `lf wt rm` with `lf wt delete`. The shared deletion operation removes remote branches too. Lower-level PR and worktree operations retain the Task outcome.
- **Operate by issue:** `lf task pr ISSUE ACTION` and `lf task sync ISSUE` address delivery operations through the Task's issue.
- **Validation:** lifecycle proofs used simulated provider responses with real disposable Git repositories and remotes; configured abandonment and sweep acceptance remain pending after installation. The later verification change records passing Python, Rust, headless Swift, architecture, multiplatform boundary, Xcode compilation, and lint checks. CI owns the remaining matrix.

## Small changes

- A per-skill check-time and context collector adds a dated baseline. Runtime savings remain unmeasured pending comparison with later Runs.
- Scratch check summaries are kept to one line to reduce repeated verification context.