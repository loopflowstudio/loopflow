# Agent startup

```sh
mkdir -p /private/benchmark
CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release -p loopflow --bin lf
cp target/release/lf /private/benchmark/lf-baseline
# Build the candidate with the same options and retain a separate immutable copy.
uv run python scripts/benchmarks/agent-startup/fixture.py \
  --source ~/.lf/loopflow.db --home /private/benchmark/dense
uv run python scripts/benchmarks/agent-startup/native.py \
  --baseline /private/benchmark/lf-baseline --candidate /private/benchmark/lf-candidate \
  --home /private/benchmark/dense --repo /private/benchmark/repo \
  --provider claude --samples 16 --output /private/benchmark/claude
```

Use a disposable checkout with the same files for every variant. Run Codex with
`--provider codex`. Run one measurement driver per Home: each selects its provider
in that Home's configuration. Keep builds and unrelated benchmarks out of the
measurement window. Copy the binary before another build can overwrite it.

`fixture.py` opens the source database read-only and uses SQLite backup. It creates
a new private directory and copies no credential files. Before any launch it
removes copied managed-account routes and their absolute provider-home references;
native providers use their ordinary authentication. The candidate CLI migrates the
private Home on its first open.
Supply the disposable checkout at `/private/benchmark/repo` before measuring.
These scripts do not create a Git worktree or alter the installed Machine.

`first_use.py` takes the same arguments and measures five fresh dense Homes per
variant. On macOS it clones a consistent private seed with APFS copy-on-write.
This means first use of the fixture, not eviction of OS or native-provider caches.
Connect is its first reconnect after preparation, not a first-ever login.

`handoff.py` replaces providers with shell stand-ins and measures only to their
readiness marker. It accepts the same baseline/candidate/Home/repo/output options
and `--samples 20`; it uses Claude. `--profile` captures macOS `sample` stacks.
Install `inferno` and `rustfilt` into a private tools directory, then render:

```sh
uv run python scripts/benchmarks/agent-startup/flames.py \
  --tools /private/benchmark/tools/bin --input /private/benchmark/profile \
  --output /private/benchmark/figures --label comparison
uv run python scripts/benchmarks/agent-startup/summarize.py \
  /private/benchmark/claude/numbers.jsonl --output /private/benchmark/summary.json
```

Keep measurements outside the checkout. SQLite files, captures and full
`sample.txt` files contain local evidence; do not commit them.
