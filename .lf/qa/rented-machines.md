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

# A live Modal rental from a branch build

`~/.modal.toml` holds the Modal login; a rental costs cents when capped
(`lf ondemand rent modal --max-usd 0.6`, ~1 min to ready). Keep the real `HOME`
(Modal, `~/.ssh`, `gh`) and a private `LF_HOME` whose `config.yaml` copies the
personal one without `machines:`/`connections:`. Run from a directory under
`~/src`: `forward.env` falls back to Doppler, which is scoped by directory.

The sandbox is x86_64 and needs the branch's Linux `lf` in `LF_RENT_LINUX_BIN`.
Without Docker, cross-build with zig: `cargo install cargo-zigbuild`, then with
`PATH=$HOME/.cargo/bin:$PATH` (Homebrew's `rustc` has no Linux std) run
`cargo zigbuild --release -p loopflow --bin lf --target x86_64-unknown-linux-gnu.2.35`
(about an hour at the Machine's `jobs = 2` from scratch; under five minutes
when `target/x86_64-unknown-linux-gnu` already holds an earlier build). That binary runs on the rental
from `~/.local/libexec/lf` behind a `~/.local/bin/lf` wrapper with
`LF_HOME=~/.lf-dev`; read its store and logs there.

From the caller, `lf --machine modal-1 -b <skill> '<message>'` records a batch
Session on the rental with the forwarded login. With the caller's
`lf server run` up, `lf session history show <id> --transcript` and
`lf session usage --session <id>` read it within about a minute. To lose the
sandbox, terminate it behind lf's back:
`uv run --with modal python -I -c "import modal; modal.Sandbox.from_id('<provider_id>').terminate()"`;
`poll()` returning 137 confirms it is gone.
Run `lf --machine modal-1 steer <id> …` from a directory outside any
repository: from inside a caller repository the rental resolves that
repository and refuses ("repository plan … has no local checkout").
A copied Session's own views (`lf session history show <id> --context`,
`--instructions`, `--input <key>`) read its capture under
`machines/<origin>/runs`; `--final` has no copy to read.

`terminate()` returns before the sandbox is gone: `poll()` reads `None` at once
and 137 a few seconds later. `lf machine end <label>` run in those seconds
already settles it without `--force`.

A private `LF_HOME` last opened by an earlier branch build refuses to open
("migration history or schema differs") once the branch edits its draft
migration; start a fresh home rather than copying the old one.

To see a start fail for want of a login, use the rental: it has `codex`
installed and no Codex login. In a fresh repository there, run
`lf session resume --prepare --json -a codex` (it prints "--agent has no
effect", which is wrong: the Session records `codex`), then from the caller
`lf --machine modal-1 steer <id> '<text>'`. The rental's `lf session list`
then says `could not start: codex has no usable login…`, and its
`lf.log` has `stopped Session not started`. A local private Machine is a poor stand-in: lf
finds `opencode` under `~/.opencode/bin` whatever `PATH` says, and an
unpinned Session resolves to it and starts.
