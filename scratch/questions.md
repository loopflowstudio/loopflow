# Decisions and remaining work — ENG-23

No human decision is required for this headless iteration.

## Decisions taken

- Use an immutable opaque source id plus a non-identifying readable name. The
  source id includes a content UUID and includes the Task UUID when available.
- Generate both migration registries from files in `build.rs`; neither feature
  branches nor canonicalization edit a shared Rust slice.
- Preserve dependency comments in canonical files and resolve source ids across
  both draft and canonical sets.
- Keep two version-named fixture generations. A release adds its target fixture
  while retaining the prior input byte-for-byte.
- Treat canonicalization as error-atomic within the release worktree. Returned
  validation/write/verification errors restore the complete before image; a
  killed process can only strand the disposable worktree.
- Remove the origin/main superset assertion. Use the merge base to reject only
  branch-authored canonical mutations and invalid release transforms.
- Ship the end-to-end boundary in the current PR rather than merge a draft file
  format whose runtime registration and release path do not yet exist.

## Known current-head failures

- Hosted `rust-test` is substantive. Reproduced locally:
  `store::migrations::tests::every_migration_file_is_registered_under_its_own_name`
  includes the `migrations/drafts` directory as the stem `drafts`, while the
  hand-maintained registry does not. Generated file registration replaces this
  test premise.
- `scratch-clear` is expected while the design artifact is under review; it is
  not the only red.
- A full local nextest run also sees two unrelated Task recovery tests inherit
  this live process's stale `LF_WAVE_ID`; each fails before its assertion. The
  migration registration failure reproduces in isolation and is the branch-owned
  Rust failure.

## Implementation order

1. Replace name-only drafts and hand-edited registry output with source-id files
   and build-generated registration.
2. Implement cross-boundary graph validation and branch-authorship checks.
3. Make canonicalization plan/stage/install transactional and add failure digest
   tests.
4. Add the two-generation fixture gate and dev-store draft lifecycle.
5. Wire bump -> canonicalize -> build/run the verifier from the changed worktree
   -> commit -> required CI -> merge -> tag, then update docs/doctor and run the
   full gates.

W2-319's promotion fence remains untouched.
