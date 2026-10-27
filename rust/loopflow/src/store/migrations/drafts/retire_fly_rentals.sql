-- Fly no longer rents. 0.14.17's Fly on-demands and rentals stay as history,
-- but lf can no longer ask Fly about one, so a rental still live is lost: its
-- guardian ends it at the deadline, and `fly apps destroy` frees what a Fly
-- app left behind. A scope that sent its Tasks to a Fly on-demand sends them
-- nowhere again.
UPDATE rentals SET state = 'lost'
    WHERE provider = 'fly' AND state NOT IN ('ended', 'lost');
UPDATE scope_defaults SET tasks_capacity = NULL
    WHERE tasks_capacity IN (SELECT name FROM ondemands WHERE provider = 'fly');
