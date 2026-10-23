-- Task events recorded before the Flow→Graph rename still say `flow_finished`.
UPDATE task_events
SET kind_json = json_remove(
    json_set(kind_json, '$.kind', 'graph_finished', '$.graph', json_extract(kind_json, '$.flow')),
    '$.flow')
WHERE json_extract(kind_json, '$.kind') = 'flow_finished';
