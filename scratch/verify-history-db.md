# Verify LOO-936 (#1965, merge fb14b674c9)

- Push CI `CI` run 44230789787 (push, fb14b674c9): success. `macOS on main` run 44230789818 (push): queued, unobserved. Merge-queue CI 44229763560: success.
- main build `lf` in isolated LF_HOME (/tmp/v936): `lf machine prune --json` → all zeros, vacuumed false; `lf machine doctor` → `ok history history.db 0.1 MB; Session history empty; turns empty`; history.db tables: history_revision, session_history, turns.
- Focused tests on main: state_reads_answer_the_same_with_history_db_deleted, retention_history_tests, retention_lifecycle_tests, session_event_lifecycle_tests, provider_transition_lifecycle_tests → 10 passed.
- Server retention: rust/loopflow/src/server/maintenance.rs runs prune at start and hourly (SystemTime deadlines), started from service.rs:181.
- Not checked here: first prune on a large real store (needs a release + real Home); before/after measurements on the real store. Process-row removal is LOO-953 (started).
