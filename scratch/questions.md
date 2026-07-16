# Open questions

## `predecessor_session_id` not yet surfaced in `TaskSession`

The carry transaction writes `predecessor_session_id` via raw SQL
(`UPDATE task_sessions SET predecessor_session_id=?2`), and the migration adds
the nullable column. But the `TaskSession` Rust struct, `task_session_params`,
and `map_task_session_row` do not map it — so the attributable-history link is
stored but not readable through the struct.

Assumption: the link is forward-looking schema (for future attribution queries)
and the read-side surfacing is a separate concern. `TaskSession` is
`Serialize`/`Deserialize` but is **not** a cross-language DTO (no Swift mirror,
no `tests/fixtures/dto/` entry), so adding the field wouldn't trigger DTO
mirror requirements — but it would change the `lf --json` task shape. Left
unsurfaced for now; the tested succession contract does not depend on it.
