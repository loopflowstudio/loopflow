# Slice 6D3 repair: finish stable-Work names and test the real census projection

## Problem

The broad review claim is still false:

- Task values remain named `parent_session` in `ops/task.rs` and
  `store/mod.rs`;
- Wave resident environment fixtures use `session_env`;
- prompt repository fixtures use `session_repo`.

The first `WorkCensusTests` proof constructs a `WorkActivity` with an injected
`launchId` and tests the one-line computed property. It does not exercise
`WorkCensus` or prove the producer invariant that only User-attention Launch
rows are openable.

## Required behavior

- Rename the listed values after reading their types: Task values to Task,
  Wave resident process environment to resident environment, and repository
  fixture values to repository names. Do not rename real provider, terminal,
  browser, URLSession, or human-session concepts.
- Replace the tautological test with a projection test that constructs the
  smallest real `WorkCensus` inputs and inspects emitted rows.
- Prove a User-attention Launch row carries its Launch id and is openable.
- Prove a non-User Launch row and non-Launch Wave/Project/Task rows carry no
  Launch id and remain view-only.
- Add no alternate action array, compatibility property, or fallback key.

## Done when

- [ ] `parent_session`, `session_env`, and `session_repo` false-Work names are
      absent in the cited contexts.
- [ ] The broad stable-Work Session audit matches its documented allowlist.
- [ ] `WorkCensusTests` constructs `WorkCensus`, not a hand-authored output row.
- [ ] The real projection proves openability for User-attention Launch only.
- [ ] Non-User Launch and non-Launch rows are proved view-only.
- [ ] Focused Rust/Swift tests, format, and Clippy pass.
- [ ] `scratch/feedback-runtime-review.md` replaces the overstated proof with
      the real projection evidence.
