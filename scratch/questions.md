# LOO-850 assumptions

`--open` keeps `-b` and `-i` foreground waiting unchanged and opens afterward.
The ordinary bare invocation returns at Graph registration and opens then.

The macOS-only `open_delivers_exact_task_to_desktop_without_preparing_its_execution`
fails on main since #1882's Desktop confirmation wait; it is left for its own Task,
not this branch.
