# Slice 6B: name Task Feedback reviewers

## Implement

- Replace Task feedback-policy vocabulary with `FeedbackReviewer::{User, Parent}`.
- Store the reviewer on each `TaskPhasePlan` and expose `lf task ... --reviewer
  user|parent`; delete the overloaded `--headless` spelling.
- Preserve the standard phase reviewers: clarify by User, pursue by Parent,
  mutate by User. An explicit reviewer override affects future checkpoints only.
- Rename persisted columns and values from require/defer policy language to
  reviewer and user/parent in one migration, with no compatibility readers.
- Delete `InteractionPolicy`, feedback-only `FlowAction` policy variants, and
  dead next-action policy helpers.

## Done when

- [ ] help and parser expose only `--reviewer user|parent`.
- [ ] default mixed reviewers and explicit overrides are behaviorally proved.
- [ ] an already-open Feedback keeps its recorded route.
- [ ] stored Task phase plans contain reviewer/user|parent with no dual reader.
- [ ] retired policy/headless symbols are absent; migration, focused behavior,
      fmt, and all-target clippy pass.
