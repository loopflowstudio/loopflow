# Resource recovery under uv run

2026-09-28 · LOO-298 · Bounded contribution requested by Jack Heart.

`uv cache prune` now has a 15-second deadline. A timeout records `timed_out`,
names an enclosing `uv run` as a possible lock holder, and continues the existing
recovery sequence. It neither bypasses the cache lock nor reports successful
pruning. The existing before/after measurements account for any reclaimed bytes.
Free disk after recovery still determines whether verification may proceed.

Only these files changed in this contribution; all remain uncommitted:

- `scripts/resource_envelope.py`: bound `_prune_uv_cache`; reuse the measured
  cache path and return an explicit incomplete result after timeout.
- `python/tests/test_resource_envelope.py`: replace the mock-call assertion with
  actual offline uv proofs for unlocked and busy private caches. Both exercise
  subsequent fixture-build recovery; the busy case retains its cache until the
  holder exits, then successfully retries. The lock holder exits after ten
  seconds even if the production timeout regresses. Cleanup owns only that
  fixture process group.
- `scratch/parallel-resource-recovery.md`: this evidence and handoff.

## Observation before editing

Read `AGENTS.md`, `TESTING.md`, the resource script and its focused tests.
Installed `uv --version` returned `uv 0.9.22 (82a6a66b8 2026-01-06)`.
`uv cache prune --help` advertises `--force` as ignoring in-use checks, but no
bounded lock-wait option. No force option was used or introduced.

A dependency-free private project, private cache, known local Python interpreter,
`UV_OFFLINE=1`, and `UV_PYTHON_DOWNLOADS=never` reproduced the issue. Inherited
`UV_*`, `LF_*`, `LOOPFLOW_*`, and `VIRTUAL_ENV` selections were removed before
supplying fixture selections. No download or shared-cache access was needed.

The command shape was:

```text
source .venv/bin/activate
python <inline TemporaryDirectory probe>
  uv run --no-config --project PRIVATE_PROJECT --python LOCAL_PYTHON python -c PROBE
    subprocess.run(["uv", "cache", "prune"], timeout=2, capture_output=True)
  uv cache prune
```

The nested prune timed out after two seconds with stderr:
`Cache is currently in-use, waiting for other uv processes to finish`.
Its enclosing uv exited zero. The same cache pruned successfully after that
parent exited. Python's subprocess timeout killed and waited for the nested
prune; no fixture process was left waiting. This distinguishes the enclosing
cache lock from slow scanning of a large cache.

## Proof after editing

The default-deadline reproduction used another private project/cache and the
same outer uv shape. Its inline child loaded the production resource module and
the existing test snapshot helpers through `runpy`, then called
`recover_resources` with a cache source and an eligible fixture build. The child
used the already-installed test interpreter, so the private project needed no
pytest installation. The outer subprocess had a 30-second deadline.

Observed result: outer exit zero in **15.15 seconds**; cache action `timed_out`
with zero bytes reclaimed; following build action `removed` with 4,096 bytes
reclaimed. The private unused archive and project source remained. After the
enclosing uv exited, a five-second-bounded `uv cache prune` exited zero and
removed the unused archive. This executes actual nesting with the production
15-second limit; the automated busy-cache test shortens only that deadline to
0.25 seconds.

Final focused commands, run from the activated `.venv`:

```sh
source .venv/bin/activate
pytest -q python/tests/test_resource_envelope.py
ruff check scripts/resource_envelope.py python/tests/test_resource_envelope.py
ruff format --check scripts/resource_envelope.py python/tests/test_resource_envelope.py
git diff --check -- scripts/resource_envelope.py python/tests/test_resource_envelope.py
```

Results: **6 tests passed in 0.43 seconds**; Ruff checks, formatting and diff
whitespace checks passed. An earlier formatting check identified only the new
argument-list layout; `ruff format` corrected it before this final proof.
Existing tests retain active builds, source, Git metadata, Run evidence and
SQLite data, and enforce the recovery-root bound. No large build or full suite
ran. Production diff: 31 changed lines; test diff: 114 changed lines, including
replacement of the old mock wiring test.

## Review and limits

The timeout bounds the prune subprocess only. It can also interrupt a legitimate
slow prune; the result says timeout rather than claiming a proven lock conflict.
It does not grant permission to delete anything uv refused. `subprocess.run`
kills and waits for its own timed-out child. Existing recovery root/byte limits,
allowlists and source/durable-data retention are unchanged. Measurements and
other commands retain their existing behavior; this is not a global recovery
deadline.

All recovery here targeted temporary fixture artifacts. No real caches,
worktrees, installed Home, Git state, provider or Task worker were mutated.
The real disk-recovery operation remains owned by the main Run. Its reported
hang is supported by the private reproduction, but this contribution does not
claim actual host capacity was recovered or the full resource CLI ran safely
against the host.

Documentation handoff for the main writer, if selected: `TESTING.md` can retain
the documented `uv run ... --recover` command and explain that busy uv pruning
times out while other eligible cleanup continues. To prune that cache, retry
`uv cache prune` directly after enclosing/other uv processes exit, or run recovery
from an activated `.venv`. No documentation outside this note was edited.
