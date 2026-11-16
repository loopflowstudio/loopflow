# Open choices (LOO-926)

- PR 1 builds on `main`, not on LOO-913's `jack-heart/cli-prox` (#1953). That
  branch is still at `606c13c2d7`, before the crate split. Whichever lands
  second removes the `prepare_launch` calls in `write::Remote::call` and the
  CLI's `ssh::run`.
- From PR 2, `lf --machine X <command>` fails when the command has no typed
  `Operation`, and the error names `ssh X lf …`. The alternative is to keep
  argv pass-through for those commands as a terminal launch. Today
  `lf --machine X install server` (`docs/troubleshooting.md:166`) is the only
  documented case found.
- The remote entry is a visible `lf server stdio`, not a hidden `__op`. An
  older lf exits 0 on any unknown `__*`, which would hide a version mismatch.
- Typed calls skip the up-front SSH probe (and its "lf versions differ"
  line); the destination checks identity, and the version is read only when
  it exits 2. The alternative keeps one extra SSH round trip per call.
