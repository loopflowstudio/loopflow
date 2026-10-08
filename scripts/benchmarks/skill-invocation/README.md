# Installed skill launch checks

```sh
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/request_mapping.py --lf target/debug/lf
uv run python scripts/test_network.py uv run --no-sync python \
  scripts/benchmarks/skill-invocation/request_mapping.py --lf target/debug/lf --flow
uv run pytest scripts/benchmarks/skill-invocation/ -q
```

The real Claude client runs against a local fake Messages API, with external
egress denied and a disposable home. The ordinary case installs a skill without
`.lf` configuration. The Flow case captures it, removes the source and adds a
same-name collision before dispatch. Both use `--no-loopflow` and check the
argument `alpha`, selected source bytes and supplied context in user messages.
The fixture reads the linked reference file itself; the provider does not read it.
A context acknowledgement cannot substitute for the command's result.

These are synthetic skills and API receipts, not unchanged third-party or live
model proofs. Default Loopflow context, quoted/multiline arguments, applied
declarations, provider asset reads, native Codex and terminal dispatch, and
third-party fidelity still need entry-point checks. Baseline reconnect and
plain-provider continuity remain in `tests/e2e/codex_connect.py` and
`tests/e2e/claude_shared_home.py`.

## Earlier provider experiments

```sh
uv run python scripts/benchmarks/skill-invocation/probe.py \
  --claude /path/to/claude --codex /path/to/codex
uv run python scripts/benchmarks/skill-invocation/continuity.py \
  --provider codex --executable /path/to/codex
```

These pre-existing standalone probes use native authentication and consume provider
usage. Their diagnostic timing and receipt checks establish neither LF launch
fidelity nor a matched cost comparison.

Jack Heart's October 8 scope correction removes busy-terminal delivery from
LOO-420. The deleted Codex queue/restart/receipt and Claude PTY/inbox probes,
failed engine-retention candidates and detailed request-mapping observations
remain at `1e4ae02a5:scripts/benchmarks/skill-invocation/README.md` and the
scripts in that commit. No native writer-custody policy was adopted. Those
observations are historical evidence, not ordinary-launch acceptance requirements.
