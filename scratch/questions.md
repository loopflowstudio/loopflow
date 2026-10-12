# Assumptions

- October 12, 2026: `ensure` and `replace` return a prepared conversation;
  `connect` launches it in the caller's terminal. Apply this consistently to
  repository, Wave and Task primary Sessions instead of keeping tmux launch
  semantics for two scopes. Jack Heart's directive leaves launch timing open.
