# Gate review: gstack workstyle import

## What was implemented

Added gstack as a first-class loopflow workstyle:

- Imported 37 gstack steps under `.lf/steps/gstack/` with namespaced invocation, for example `lf gstack:office-hours`.
- Added gstack flows under `.lf/flows/gstack/` for sprint planning, parallel review, and manual plan review.
- Added a Python converter at `python/loopflow/workstyle/convert.py` to sync upstream gstack skills into loopflow step files and generated directions.
- Extended Rust step and flow discovery so namespaced steps and hyphenated namespaced flows appear in `lf --list` and resolve during flow expansion.
- Added `lf op gstack sync|diff|list` for workstyle maintenance.
- Added gstack and openclaw built-in directions, plus starter wave docs for gstack and runboard.

## Key choices

- Namespaced steps use colon syntax at the CLI (`gstack:office-hours`) while mapping to `.lf/steps/gstack/office-hours.md` on disk. This keeps imported workstyles distinct from built-in loopflow steps.
- Namespaced flows use hyphenated display names (`gstack-sprint`) so they fit existing flow listing and invocation patterns.
- Imported gstack telemetry and Claude-specific wrapper sections are stripped during conversion. Loopflow should run the prompts, not inherit upstream hook telemetry or tool-specific shell wrappers.
- The gstack review flow now uses the built-in `synthesize` step. The previous custom `gstack:review-synthesize` reference did not exist and would fail after the parallel review branches completed.
- GStack and OpenClaw direction text was cleaned to avoid identity framing in built-in directions while preserving the imported intent.

## How it fits together

The Python converter turns an upstream skill repo into loopflow step files and a `workstyle.yaml` manifest. Rust discovery treats `.lf/steps/<prefix>/*.md` as a namespaced skill source, and flow loading resolves `.lf/flows/<prefix>/<name>.yaml` as `<prefix>-<name>`. The `lf op gstack` commands update the cache, rerun the converter, and report changed step files.

## Risks and bottlenecks

- The imported prompt set is large. Reviewers should focus on converter behavior, discovery semantics, and representative generated output rather than reading every imported prompt line-by-line.
- `lf op gstack sync` shells out to `git` and `uv run python -m loopflow.workstyle.convert`; failures depend on local Git, uv, network, and upstream repo availability.
- The converter deliberately strips specific upstream sections by regex. Upstream format changes may require updating those cleanup rules.
- Namespaced flow discovery accepts hyphenated names by splitting on the first hyphen. That matches `gstack-sprint`, but future prefixes with hyphens may need a more explicit lookup scheme.

## What's not included

- No automatic scheduled sync from upstream gstack.
- No compatibility layer for old imported prompt formats beyond the current converter cleanup rules.
- No exhaustive rewrite of imported prompt prose into native loopflow prompt style. The gate pass only cleaned built-in directions and fixed an invalid flow reference.

## Validation

- `cargo fmt --check` — passed.
- `cargo clippy -p loopflow -- -D warnings` — passed.
- `cargo test -p loopflow` — passed, 933 unit tests plus integration/doc tests, 2 ignored.
- `uv run ruff check python/loopflow/workstyle/convert.py python/tests/test_workstyle_convert.py` — passed.
- `uv run pytest python/tests/ -q` — passed, 120 tests.
- `uv run pytest python/tests/test_workstyle_convert.py -q` — passed, 3 tests.
- `cargo run -q -p loopflow --bin lf -- --list` — verified gstack flows and external skills appear.
- `cargo run -q -p loopflow --bin lf -- op gstack list` — verified 37 gstack steps and 3 flows are listed.
