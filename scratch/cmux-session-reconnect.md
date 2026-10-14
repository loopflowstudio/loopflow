# cmux reconnect incident

Jack Heart reported `lf session connect aaa3bce933ca4809bddf84233dc51dbc`
failing with `Previous AgentProcess has no observed exit`, and a perceived
`lf incident -i -c` hang on Oct 13. Jack later directed work toward durable fixes
because another usable conversation was open; that pane was left untouched.
The current design and remaining work are in `infra-bugs.md`.

## Causal evidence

Installed v0.13.13 came from `6935bfbadbc2cecca1321dda4bd6668b3d42a062`, before
#1542 (`def8881d82`) added positive death observation for interactive providers.
The original provider and launcher PIDs were absent, but its record had no exit.
After a dry run found no live orphan groups, `lf monitor prune --json` retired it
with zero errors/terminated groups and no invented outcome. Reconnect advanced
past the stale-exit refusal, then hit a separate startup timeout. `lf install`
reported no newer published release at the time.

The retry held Bash rather than Codex and recorded the exact 30-second readiness
timeout. Its provider briefly outlived the launcher, then disappeared without a
recorded exit. Replaying only the installed cmux scanner with those exact argv
values took 26.617s. `codex_launch_has_subrouter_provider` stripped whitespace from
all `-c` values, including 3,087 hook characters and 8,027 instruction characters
unrelated to provider selection. This supports wrapper delay consuming the startup
budget and launcher timeout skipping normal exit recording; the original Oct 13
18:46 launch was not sampled live. Removing the stale-exit guard alone cannot fix
the wrapper delay.

The prepared `cmux-config-scan.patch` normalizes the config key first, examining
values only for `model_provider`. Replay fell to 0.005s, eight selection cases
passed against both scanners, and the complete patched wrapper reached an isolated
stand-in in 0.210s. It was neither installed nor submitted upstream. Reproduce
without an account, provider or cmux effects:

```sh
uv run python scratch/check-cmux-config-scan.py \
  /Applications/cmux.app/Contents/Resources/bin/cmux-codex-wrapper \
  scratch/cmux-config-scan.patch
```

Faster wrapper parsing alone did not prove recovery: other live Codex Sessions
also lacked native history, and an early actual-provider PTY probe panicked in
crossterm (`reader source not set`). Later protocol implementation and real cmux
acceptance are documented in `infra-bugs.md`; no installed bundle was modified.

On Oct 14 Jack reported another 10+ second `lf resume` wait. Launcher/provider
start timestamps differed by one second, but did not measure wrapper completion
or terminal readiness. Installed argv was still 10,862 bytes. This is consistent
with the wrapper finding, without proving that restart's exact cause or duration.
The original perceived incident-command hang was never reproduced.

Full IDs, executable digest, intermediate probes and earlier check results:
`5cbabcb81a:scratch/cmux-session-reconnect.md` and
`5cbabcb81a:scratch/infra-bugs.md`. The initial cmux 0.97-second readiness estimate
was explicitly invalidated because it matched old screen text. Release delivery
and installed-path verification remain outstanding.
