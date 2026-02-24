# Open questions

- Session config still accepts `max_turns` and `yolo_mode`, but the new `PreparedPrompt` trait handoff does not currently pass these through to harnesses. Assumption: acceptable for this first draft; can be added to `PreparedPrompt` or a separate runtime option object in follow-up.
- `RepoState.chatState(for:)` now defaults to `step: design` for all chat sessions (including wave detail chat tab). Assumption: acceptable interim behavior until per-tab/per-wave step selection is specified.
