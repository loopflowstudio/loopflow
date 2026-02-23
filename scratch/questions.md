# Open questions

- Docker fork branches still create host git worktrees (instead of empty host dirs) so `build_step_prompt` can resolve step files and context logs before container launch. If we want true host-dir placeholders, we likely need a pre-prompt container sync or a prompt build path that does not depend on host worktree content.
