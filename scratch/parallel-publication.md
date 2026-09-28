# Publication identity before fallible follow-up

2026-09-28 · LOO-298 · Bounded unapplied patch for the managed worker.

[parallel-publication.patch](parallel-publication.patch) changes only
`ops/pr.rs`, `ops/task.rs`, and `tests/task_pr_authority_tests.rs`. It was
prepared from temporary copies of the dirty source at HEAD
`03330279f34a75480465a6c8239935c3ac4638ee`. No executable working file was edited;
no build, behavioral test, provider call, Git mutation, or configured Home ran.
The original cause of PR #1296's missing association remains unknown.

## Ordering

| Path | Before | Proposed |
| --- | --- | --- |
| Creation | create → follow-up read → ready → attach | create → attach acknowledged URL/number → follow-up read → attach observed head → ready |
| Existing PR | read → edit → ready → attach | read → attach known identity → edit → ready |
| Attachment | reconcile merge → Linear link → save identity/linkage → event | save identity → event → reconcile merge → Linear link → save linkage |

Creation supplies no observed head: the first attachment stores `head_sha=None`.
A failed GitHub read or readiness operation still returns its error. Linear's
existing degraded-link result is retained on the same Task PR. No additional
ledger, fallback read, fabricated head, or new production abstraction is added.
A successful follow-up read may call the existing idempotent Linear linker a
second time; its saved attachment/comment IDs remain the retry inputs.

The publication request, exact Task resolution, branch check, mutation lock,
remote-head checks and merge invalidation rules remain. Missing head evidence
still refuses changes to a head-pinned merge request, after retaining acknowledged
identity. The patch leaves the stacking correction intact: generic PR updates
do not overwrite `parent_pr_id`, and publication rereads its base under the lock.
It does not modify the store writer.

## Authored proof, not executed

- `acknowledged_creation_survives_failed_read_and_retries_without_duplicate`:
  simulated GitHub initially lists nothing, acknowledges PR 7, then fails its
  follow-up read. Assert the command error, original Task PR ID/parent, URL/number,
  unknown head and recorded Linear degradation. Make that same PR visible and
  retry: assert the same local PR, observed head, and exactly one creation.
- `existing_pr_identity_survives_readiness_failure`: simulated GitHub reports
  an existing draft, then refuses readiness. Assert the error and retained local
  PR identity, URL/number/head, with no creation.

The existing fixture has no Wave configuration, so its Linear linkage failure
provides the smallest degradation check without adding a production test seam.
It does not simulate interruption inside a Linear request. Source ordering puts
identity before that request; actual crash/final-writeback fault injection remains
unproven. A crash before GitHub returns its creation response also remains unknown;
retry retains the existing fresh `find_open_pr` reconciliation. Identity and the
PrOpened event are still separate writes; this patch does not claim atomic event
publication or exactly-once external effects.

After integration, main should run the existing authority suite once under its
serialized build slot, resource preflight and scrubbed private-Home runner:

```sh
cargo nextest run -p loopflow -j 4 --test task_pr_authority_tests --test-threads 4 --no-fail-fast
```

Keep the suite's same-head/merge-request checks and the separately owned stacking
preservation proof. Use main's combined formatting/Clippy pass; this contribution
has no build slot and does not claim type checking or behavior passes.

## Checks performed

All three final proposed Rust files parsed through `rustfmt --emit stdout` with
`skip_children=true`. The fixture shell passed `sh -n`. An unrelated formatting
hunk was removed. Final `git apply --check scratch/parallel-publication.patch`
passed against the current dirty source; each target still matched its temporary
input copy at that check. The patch is **unapplied**: 211 insertions, 40 deletions
across three files. These are syntax/context checks only.

Patch SHA-256:
`6dac9089e75698ab33929cbddfbd636aba578b3f2adc0e04db9f014fa52da02e`.
