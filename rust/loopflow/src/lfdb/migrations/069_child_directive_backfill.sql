INSERT INTO child_directives (
    id, target_kind, target_id, version, kind, text, source_json,
    command_id, issued_at, applied_at, incorporated_at, incorporated_summary
)
SELECT
    'dir_' || lower(hex(randomblob(16))),
    'project',
    session.id,
    1,
    'initial',
    'Resume existing Linear Project ' || session.project_name || '.' ||
        char(10) || char(10) || session.project_context,
    '{"kind":"system"}',
    NULL,
    session.created_at * 1000000000,
    NULL,
    NULL,
    NULL
FROM project_sessions AS session
WHERE session.current_directive_version = 0
  AND NOT EXISTS (
      SELECT 1
      FROM child_directives AS directive
      WHERE directive.target_kind = 'project'
        AND directive.target_id = session.id
  );

UPDATE project_sessions
SET current_directive_version = 1
WHERE current_directive_version = 0
  AND EXISTS (
      SELECT 1
      FROM child_directives AS directive
      WHERE directive.target_kind = 'project'
        AND directive.target_id = project_sessions.id
        AND directive.version = 1
  );

INSERT INTO child_directives (
    id, target_kind, target_id, version, kind, text, source_json,
    command_id, issued_at, applied_at, incorporated_at, incorporated_summary
)
SELECT
    'dir_' || lower(hex(randomblob(16))),
    'task',
    session.id,
    1,
    'initial',
    'Resume existing Linear task ' || session.issue_identifier || ': ' ||
        session.issue_title ||
        CASE
            WHEN trim(session.issue_description) = '' THEN ''
            ELSE char(10) || char(10) || session.issue_description
        END,
    '{"kind":"system"}',
    NULL,
    session.created_at * 1000000000,
    NULL,
    NULL,
    NULL
FROM task_sessions AS session
WHERE session.current_directive_version = 0
  AND NOT EXISTS (
      SELECT 1
      FROM child_directives AS directive
      WHERE directive.target_kind = 'task'
        AND directive.target_id = session.id
  );

UPDATE task_sessions
SET current_directive_version = 1
WHERE current_directive_version = 0
  AND EXISTS (
      SELECT 1
      FROM child_directives AS directive
      WHERE directive.target_kind = 'task'
        AND directive.target_id = task_sessions.id
        AND directive.version = 1
  );
