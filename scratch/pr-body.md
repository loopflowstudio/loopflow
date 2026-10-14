Continuing work after a merged PR previously required another Task or manual branch cleanup. This PR lets you publish the next change from the same checkout while preserving earlier PR history and carrying follow-up work past squash merges.

## What changes

- Publication discovers previous merges and creates a new current PR, including when the remote branch was pruned. Retries reuse that PR.
- Successor creation atomically preserves the settled PR, clears earlier completion intent and reopens a completed Task. An earlier landing cannot complete the new delivery.
- Branch continuation preserves local edits and already-pushed follow-up commits. Remote history is replaced only when the exact delivered head is still present, using a lease; divergent or concurrent remote work remains intact.
- `lf pr submit` creates a missing PR without requiring `--create-pr`. Repeating `lf land` prepares another PR when new work exists and remains a no-op for an unchanged merged delivery.
- Task checkout names use the same normalization as `lf wt create`. Checkout creation or restoration precedes explicit agent selection, and reconciliation skips deleted directories while retaining Task, PR and landing intent.

Publication can reopen a completed Task; launching completed Tasks is unchanged. Integrating unrelated upstream changes still requires sync.

## Checks

Recorded branch verification uses real Git repositories and the built CLI with simulated GitHub responses:

- Broad Rust run: 2,601 passed, 3 failed and 22 skipped. After repairs, the affected suites passed 214/214; data-clone checks passed 4/4. The complete broad run was not repeated.
- Formatting, Clippy, architecture checks, test-inventory checks and diff checks passed. Website checks passed 75/75; skill-alignment checks passed 4/4.
- The aggregate runner stopped at its resource-inventory timeout; direct checks supplied the results above. Hosted full-matrix CI remains pending after scratch cleanup; skipped CI is not a full pass.

## Try it

Suggested live walkthrough after installing a release containing this change: squash-merge a Task’s PR on GitHub, return to its checkout, make a follow-up change and run `lf pr publish`. The new PR should contain only the follow-up change, with the previous PR retained in Task history. Publish again and confirm it updates the same new PR.

An installed Fantasia release replay and live GitHub merge lifecycle remain unverified; the recorded proofs simulated GitHub responses.

### Hosted CI follow-up

The first hosted integration run exposed a platform-dependent fixture assumption: macOS temporary-directory aliases differed from Git’s canonical checkout path, while Linux returned successful primary-checkout retention. The fixture now records Git’s canonical path and requires successful completion before testing reopening; no production behavior changed in this repair. All 22 PR-range tests passed locally with `/tmp` aliases, along with build, all-target Clippy, and formatting. Hosted CI is rerunning for the repaired head.
