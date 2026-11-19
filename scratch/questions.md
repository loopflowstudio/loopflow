# Open choices (LOO-984)

- Lost-thread decision moved from `lost_thread` (open path, #1972) into `resume_native_session`, and `lost_thread` deleted. The alternative was to keep `lost_thread` and delete only the guards. Revisit if a recovery path must create its input under a request id (`fresh-thread:`) for crash idempotence rather than under the launch lock.
- A lost thread now gets a new input through `append_input`, not the same input restarted with `--session-id <same id>`. Real Claude accepts reusing the id, but restarting a consumed input runs against capture and manifest state.
