# LOO-582 assumptions

Jack Heart rejected `--new` and clarified that adoption must never be the
default, including with `--run`. Creation files new work; --run starts it;
explicit --session/--worktree selects adoption. The accepted correction is at
the top of `scratch/loo-582.md`; the working tree implements it; publication remains pending.

Implementation assumption (2026-11-03): an explicit move waits until the live
client exits and the next connection starts. Promotion never stops a live client
or submits/replays input. CLI and Desktop explain that continuation boundary.

Implementation assumption (2026-11-03): `--worktree --run` keeps ordinary Task
conversation admission after explicit checkout adoption. Creation itself does not
bind the invoking Session; starting the Task may open a conversation associated
with its now-adopted checkout. The new fixture checks both boundaries.
