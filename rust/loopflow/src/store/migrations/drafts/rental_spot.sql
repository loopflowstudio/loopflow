-- depends_on: scope_tasks_capacity
-- Hardware says whether it is interruptible (spot); everything rented before
-- this was not.
UPDATE ondemands SET template = json_set(template, '$.spot', json('false'));
UPDATE rentals SET hardware = json_set(hardware, '$.spot', json('false'));
