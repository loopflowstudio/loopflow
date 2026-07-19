# Slice 6A: make Feedback presentation safe

## Implement

- Make `lf work feedback` presentation-only.
- Delete `continue_on_success`, `continue_on_exit`, the hidden feedback-exit
  guard, exit policy/retry/lock state, and conditional continuation store API.
- Keep `lf work continue` as the only Feedback-closing operation.
- Delete Feedback escalation command/store transition/receipt; Task Feedback
  chooses User or immediate Parent when it opens and never changes route.
- Update Swift launch argv, help, docs, parser/store/behavior tests with no
  compatibility aliases.

## Done when

- [ ] presentation cannot advance flow on any process exit or signal.
- [ ] explicit `work continue` is the only close path.
- [ ] escalation API/state is absent.
- [ ] parser rejects retired flags/commands and focused Rust/Swift tests, fmt,
      and clippy pass.
