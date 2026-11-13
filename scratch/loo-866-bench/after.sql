WITH windowed(seq) AS (
                SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?7
                UNION SELECT +seq FROM session_events WHERE ?7 IS NULL AND kind='captured' AND observed_at>=?5
                UNION SELECT +c.seq FROM session_events o CROSS JOIN session_events c ON c.kind='captured'
                    AND c.receipt_key=substr(o.receipt_key,1,length(o.receipt_key)-14)
                    WHERE ?7 IS NULL AND o.kind='observed' AND substr(o.receipt_key,-14) IN (':manifest.json',':terminal.json')
                    AND o.observed_at>=?5 AND (?6 OR substr(o.receipt_key,-14)=':manifest.json')
                UNION SELECT origin.captured_event FROM session_events completion CROSS JOIN session_events origin
                    ON origin.session_id=completion.session_id AND origin.provider_thread=completion.provider_thread
                    AND origin.provider_turn=completion.provider_turn AND origin.kind='started'
                    WHERE ?7 IS NULL AND ?6 AND completion.kind IN ('started','usage','completed','output')
                    AND completion.kind='completed' AND completion.observed_at>=?5 AND +origin.captured_event IS NOT NULL
                UNION SELECT CASE ran.kind WHEN 'captured' THEN ran.seq ELSE ran.captured_event END
                    FROM processes exited CROSS JOIN session_events ran ON ran.lf_process_id=exited.id
                    WHERE ?7 IS NULL AND ?6 AND exited.completed_at>=?5 AND ran.kind IN ('captured','started')
                    AND (ran.kind='captured' OR ran.captured_event IS NOT NULL)),
            -- A native turn starts in the window only if all its events fall in it;
            -- one its Process completed in the window may have started earlier.
            turns(session_id,provider_thread,provider_turn) AS (
                SELECT +session_id,+provider_thread,+provider_turn FROM session_events
                    WHERE ?7 IS NULL AND kind IN ('started','usage','completed','output') AND observed_at>=?5
                UNION SELECT +ran.session_id,+ran.provider_thread,+ran.provider_turn
                    FROM processes exited CROSS JOIN session_events ran ON ran.lf_process_id=exited.id
                    WHERE ?7 IS NULL AND ?6 AND exited.completed_at>=?5 AND ran.kind='started' AND ran.captured_event IS NULL),
            inputs AS (
                SELECT i.seq AS captured,i.receipt_key AS input_id,i.session_id,json_extract(i.payload,'$.caller_key') AS caller_input_id,
                    COALESCE(m.observed_at,i.observed_at) AS started,
                    CASE WHEN m.seq IS NOT NULL THEN m.task_id ELSE i.task_id END AS task_id,
                    CASE WHEN m.seq IS NOT NULL THEN m.wave_id ELSE i.wave_id END AS wave_id,
                    CASE WHEN attached.completed_at IS NOT NULL THEN attached.completed_at
                    WHEN terminal.seq IS NOT NULL AND NOT (
                        json_valid(terminal.payload) AND CASE WHEN json_valid(terminal.payload) THEN
                        COALESCE(json_type(terminal.payload,'$.evidence.outcome')='text',0)
                        AND unixepoch(json_extract(terminal.payload,'$.evidence.ended_at')) IS NOT NULL ELSE 0 END
                    ) THEN NULL WHEN EXISTS (
                        SELECT 1 FROM session_events origin WHERE origin.session_id=s.id
                        AND origin.captured_event=i.seq AND origin.kind='started' AND NOT EXISTS (
                            SELECT 1 FROM session_events done WHERE done.session_id=origin.session_id
                            AND done.provider_thread=origin.provider_thread AND done.provider_turn=origin.provider_turn
                            AND done.kind='completed')
                        ) THEN NULL ELSE
                        COALESCE(terminal.observed_at,(
                            SELECT MAX(done.observed_at) FROM session_events origin JOIN session_events done
                            ON done.session_id=origin.session_id AND done.provider_thread=origin.provider_thread
                            AND done.provider_turn=origin.provider_turn AND done.kind='completed'
                            WHERE origin.session_id=s.id AND origin.captured_event=i.seq AND origin.kind='started')) END AS ended, NULL AS thread, NULL AS turn,
                    MAX(i.observed_at,COALESCE(m.observed_at,0),
                        COALESCE((SELECT MAX(o.observed_at) FROM session_events o WHERE o.captured_event=i.seq),0),
                        COALESCE((SELECT MAX(d.observed_at) FROM session_events origin JOIN session_events d
                            ON d.session_id=origin.session_id AND d.provider_thread=origin.provider_thread
                            AND d.provider_turn=origin.provider_turn
                            WHERE origin.session_id=s.id AND origin.captured_event=i.seq AND origin.kind='started'),0)) AS latest
                FROM windowed CROSS JOIN session_events i ON i.seq=windowed.seq JOIN agent_sessions s ON s.id=i.session_id
                LEFT JOIN session_events latest_turn ON latest_turn.seq=(
                    SELECT origin.seq FROM session_events origin WHERE origin.session_id=i.session_id
                    AND origin.captured_event=i.seq AND origin.kind='started' ORDER BY origin.seq DESC LIMIT 1)
                LEFT JOIN processes attached ON attached.id=CASE WHEN latest_turn.seq IS NULL
                    THEN i.lf_process_id ELSE latest_turn.lf_process_id END
                LEFT JOIN session_events m ON m.session_id=s.id AND m.kind='observed'
                    AND m.receipt_key=i.receipt_key||':manifest.json'
                LEFT JOIN session_events terminal ON terminal.session_id=s.id AND terminal.kind='observed'
                    AND terminal.receipt_key=i.receipt_key||':terminal.json'
                WHERE i.kind='captured' AND (?7 IS NULL OR i.receipt_key=?7)
                    -- `eligible` keeps only these when finished inputs are excluded;
                    -- filtering here spares the per-input subqueries on older ones.
                    AND (?6 OR COALESCE(m.observed_at,i.observed_at)>=?5) AND
                    (?7 IS NOT NULL OR m.seq IS NOT NULL
                    OR (i.receipt_key=(SELECT receipt_key FROM session_events WHERE seq=s.current_capture) AND s.input_published=1)
                    OR EXISTS(SELECT 1 FROM session_events origin WHERE origin.session_id=i.session_id
                        AND origin.captured_event=i.seq AND origin.kind='started'))
                UNION ALL
                SELECT NULL,NULL,e.session_id,NULL,e.started,origin.task_id,origin.wave_id,
                    COALESCE(MAX(attached.completed_at),e.completed),e.provider_thread,e.provider_turn,e.latest
                FROM (SELECT turn_event.session_id,turn_event.provider_thread,turn_event.provider_turn,
                        MIN(observed_at) AS started,
                        MAX(CASE WHEN kind='completed' THEN observed_at END) AS completed,
                        MAX(observed_at) AS latest
                    FROM turns CROSS JOIN session_events turn_event USING (session_id,provider_thread,provider_turn)
                    WHERE turn_event.kind IN ('started','usage','completed','output') AND turn_event.captured_event IS NULL
                    GROUP BY turn_event.session_id,turn_event.provider_thread,turn_event.provider_turn
                    HAVING ?6 OR MIN(observed_at)>=?5) e
                LEFT JOIN session_events origin
                    ON origin.session_id=e.session_id AND origin.provider_thread=e.provider_thread
                    AND origin.provider_turn=e.provider_turn AND origin.kind='started'
                LEFT JOIN processes attached ON attached.id=origin.lf_process_id
                WHERE origin.captured_event IS NULL
                GROUP BY e.session_id,e.provider_thread,e.provider_turn)
                , eligible AS MATERIALIZED (
                SELECT captured,input_id,session_id,caller_input_id,started,task_id,wave_id,ended,thread,turn,latest, SUM(ended IS NULL) OVER () AS unfinished,
                    ROW_NUMBER() OVER (PARTITION BY ended IS NULL ORDER BY started DESC,input_id DESC,session_id,thread,turn) AS ordinal,
                    COUNT(*) OVER () AS total
                FROM inputs
                WHERE (?1 IS NULL OR wave_id IN (SELECT id FROM wave_addresses WHERE id=?9 OR slug=?1)
                    OR EXISTS (SELECT 1 FROM session_events origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured AND origin.kind='started'
                        AND origin.wave_id IN (SELECT id FROM wave_addresses WHERE id=?9 OR slug=?1)))
                AND (?2 IS NULL OR task_id IN (SELECT t.id FROM projects p CROSS JOIN tasks t ON t.project_id=p.id
                    WHERE p.id=?10 OR p.project_slug=?2 OR p.external_project_id=?11)
                    OR EXISTS (SELECT 1 FROM session_events origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured AND origin.kind='started'
                        AND origin.task_id IN (SELECT t.id FROM projects p CROSS JOIN tasks t ON t.project_id=p.id
                            WHERE p.id=?10 OR p.project_slug=?2 OR p.external_project_id=?11)))
                AND (?3 IS NULL OR task_id IN (SELECT id FROM tasks WHERE id=?3 OR issue_identifier=?3 OR external_issue_id=?3)
                    OR EXISTS (SELECT 1 FROM session_events origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured AND origin.kind='started'
                        AND origin.task_id IN (SELECT id FROM tasks WHERE id=?3 OR issue_identifier=?3 OR external_issue_id=?3)))
                AND (?4 IS NULL OR caller_input_id=?4 OR caller_input_id IN (SELECT receipt_key FROM session_events WHERE kind='captured' AND session_id=?12))
                AND (started>=?5 OR (?6 AND ended>=?5)))
                SELECT eligible.session_id,eligible.input_id,eligible.caller_input_id,started,eligible.task_id,eligible.wave_id,
                    (SELECT slug FROM wave_addresses WHERE id=eligible.wave_id),
                    s.current_capture,s.cwd,s.repo,s.skill,s.provider,s.model,s.interactive,
                    total > MAX(?8,unfinished),COALESCE((SELECT json_extract(payload,'$.work_source') FROM session_events WHERE seq=eligible.captured),
                        CASE WHEN eligible.captured=s.current_capture AND s.bound_at IS NULL THEN s.work_source END),
                    eligible.captured,eligible.thread,eligible.turn,eligible.latest
                FROM eligible CROSS JOIN agent_sessions s ON s.id=eligible.session_id
                WHERE (?8 IS NULL OR ended IS NULL OR ordinal<=MAX(?8-unfinished,0))
                ORDER BY started DESC,eligible.input_id DESC,eligible.session_id,eligible.thread,eligible.turn
