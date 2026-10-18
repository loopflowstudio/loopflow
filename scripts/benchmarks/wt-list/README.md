# `lf wt list` performance

```sh
uv run python scripts/benchmarks/wt-list/profile.py --lf target/release/lf --repo ~/src/loopflow --samples 20
uv run python scripts/benchmarks/wt-list/profile.py --lf target/release/lf --repo ~/src/loopflow --samples 20 --offline
uv run python scripts/benchmarks/wt-list/profile.py --lf ~/.local/bin/lf --repo ~/src/loopflow --home main   # installed CLI, main Home
uv run python scripts/benchmarks/wt-list/profile.py --lf target/release/lf --repo ~/src/loopflow --store ~/.lf/loopflow.db   # copy of a real store
```

```sh
lf wt timing            # real invocations on this machine: count, median/p95, failures, version
lf wt timing --json
```

`profile.py` stages runs and counts processes. `lf wt timing` reports what
ordinary use measured.

Each sample is one default (read-only) listing. Git's trace2 stream counts and
times every Git process; a shim times `gh`; `lf home id` on the same executable
and Home gives the cost of process start, Exec admission and both ledger writes
with no repository work. `--home fresh` (default) points `LF_HOME` at a new empty
directory so a branch build never opens the main Home. `--store` copies a
database into that fresh Home first (a copy-on-write clone on APFS), so the run
pays for a store of real size and writes only to the copy. `--offline` routes
every remote call to a closed local port.

Keep run output outside the checkout. Compare matching hosts, stores and builds.
