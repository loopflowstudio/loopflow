-- Fly is no longer a rental provider, and no reader parses `fly`. 0.14.17
-- could write Fly on-demands and rentals, so they go, along with any scope
-- that sent its Tasks to one. A Fly app left behind bills only its IPv4 until
-- `fly apps destroy` removes it.
DELETE FROM rentals WHERE provider='fly';
UPDATE scope_defaults SET tasks_capacity = NULL
    WHERE tasks_capacity IN (SELECT name FROM ondemands WHERE provider='fly');
DELETE FROM ondemands WHERE provider='fly';
