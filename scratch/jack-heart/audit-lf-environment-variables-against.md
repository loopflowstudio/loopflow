# LOO-341: audit lf environment variables

Jack Heart requested the audit on 2026-09-30. The durable inventory lives in
`docs/architecture/environment.md`; this file holds the cut list and what is left.

## Findings that drive the cuts

- `LF_CONTROL_BIN`, `LF_CONTROL_HOME`, `LF_CONTROL_DB_PATH` have no production
  reader. Rust only removes them; tests and scripts still set them.
  `scripts/publish_release.py` sets `LF_CONTROL_DB_PATH` to keep candidate
  preflight off the real store, which does nothing.
  `scripts/bootstrap-cron-host.sh` and `scripts/resource_envelope.py` still
  prefer them over `LF_HOME`, so a stale value picks a Home lf itself ignores.
- `LF_DB_PATH` is redundant with `LF_HOME`. It is ignored under the main Home
  and honoured under a custom one, which is the release/dev split that
  triggered the audit. Every production writer sets it to `$LF_HOME/loopflow.db`.
- Scrubbed or forwarded but never read: `LF_RUN_CONTEXT`, `LF_TASK_ORIGIN`,
  `LF_HUMAN_SESSION_RUN_BIND`, `LF_WORKTREE_WRITER_ID`, `LF_SSH_TARGET`,
  `LF_LINEAR_WEBHOOK_SECRET`, `LF_LINEAR_VIEWER_ID`, `LF_GITHUB_WEBHOOK_SECRET`,
  `LF_GITHUB_WEBHOOK_URL`, `LF_LFD_ALLOW_NON_LOOPBACK`. Test-only residue:
  `LF_PARENT_RUN_ID`, `LF_RUN_LEASE`, `LF_AGENT_INVOCATION_ID`,
  `LF_INSTALL_PROMOTE_HOP`, `LF_TASK_{SESSION_ID,GENERATION,LEASE_TOKEN}`,
  `LF_PROJECT_{SESSION_ID,GENERATION,LEASE_TOKEN}`.
- The tmux leak: a session shell unsets Exec context, but the `tmux` client
  that may start the server keeps it, so the server's global environment
  carries `LF_BIN`, `LF_FLOW_STEP`, `LF_RUN_ID`, `LF_RUN_DIR` into every later
  session a person opens by hand.
- `engine/config.rs` resolves the global config from `LF_HOME` with its own
  rules (empty value accepted, different home lookup) instead of the store's.
- `ops/run.rs` pushes `LF_HOME` and `LF_DB_PATH` twice for a Task worker.

## Delete — do not maintain

- `store::CONTROL_BIN_ENV`, `CONTROL_HOME_ENV`, `CONTROL_DB_PATH_ENV` and every
  set, scrub, capture and assertion of `LF_CONTROL_*` in tests and scripts.
- `LF_DB_PATH`: the reader in `store::database_path_from_env`, the escape check
  in `resolve_database_path`, every production writer, `db_path` on
  `ChildExecutionContext`, `cron::HostSpec` and the CI-watch launch spec, the
  plist keys, and test/script uses. The database is `$LF_HOME/loopflow.db`.
- The never-read names listed above, from scrub lists, forwarded-authority
  names, test ambient lists and scripts. `git_operation::LEGACY_WORKTREE_WRITER_ID_ENV`
  and `process::SSH_TARGET_ENV` go with them.

Preserve: explicit `LF_HOME` experiments, dev builds refusing the production
database, `LF_BIN` under a custom Home, session shells clearing inherited Exec
context before applying their own.

## Consolidate

- One list of process-context names in `engine/process.rs` produces both the
  session shell's `unset` and the `tmux` client's removed environment.
- `engine/config.rs` uses `store::lf_home_dir()`.
- Task worker environment sets each Home variable once.
- Desktop's inherited-context scrub covers the same Exec context names.

## Kept, with a reason

See the inventory. `LF_BIN` stays: under the main Home lf ignores it and uses
the installed CLI; under a custom Home it names the experiment's binary.

## Remaining

- Checks (2026-10-01): `cargo clippy --all-targets -- -D warnings` clean;
  `cargo test -p loopflow --lib` 1583/1586 with ambient `LF_*` removed; focused
  integration tests (one_home, doctor, cli_discovery, exec_ownership,
  global_commands) and the four touched pytest files pass. Full matrix: gate/CI.
- The three lib failures (`store::migrations::tests::legacy_persisted_json_upgrades_to_typed_stable_tasks`,
  `store::sqlite::planning::tests::migration_preserves_planning_identity_and_removes_snapshot_storage`,
  `store::tests::sqlite_open_migrates_existing_plaintext_provider_tokens`) fail
  identically on main at `3d1f76780`; they are not from this branch.
- Follow-up, unfiled: `LF_RUN_ID` presence deciding behaviour (see questions).
