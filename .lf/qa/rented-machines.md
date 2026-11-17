Read when the change rents a Machine or moves Work onto one.

# Rented Machines without a provider

Renting runs the provider bridge as `uv run … python -c <bridge> <provider> <op>`
and reaches the sandbox with `ssh`, both from `PATH`. Stand-ins on a private
`PATH` drive `lf machine ondemand rent`, `lf task move --machine` and
`lf machine end` with no account or cloud: `uv` answers `create` with a unique
`id` and an SSH `route` (`qa@sandbox<n>.invalid`), `get` with the sandbox or
`null`, and `delete` with `null`; `ssh` takes its last argument as the remote
command and runs it under a separate `HOME`/`LF_HOME` per sandbox, with the
branch `lf` on that `PATH`. Exit 0 for the `install.sh` command and print
`0\n0\n` for the `pgrep` holdings script. Each sandbox needs its own home: one
Machine identity behind two rentals fails `rentals.machine_id`. Set any
placeholder `DAYTONA_API_KEY` and an SSH key in the private `HOME`; stub
`ssh-keygen -R`, which would edit the real `known_hosts`. To land a write between
a preflight read and its transaction, serve a repository's `.lf/config.yaml`
through a FIFO and act from another repository at the first open.
