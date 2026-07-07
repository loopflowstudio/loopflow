## Try it!

```bash
swift test --package-path swift -Xswiftc -gnone --filter WavePlanParserTests
swift test --package-path swift -Xswiftc -gnone --filter PortfolioRepoStateTests
uv run python scripts/test.py
```

Launch Concerto against this repo and select the `concerto` wave. The detail
pane shows the Objective from `wave/concerto/GOAL.md` and the five live projects
from `wave/concerto/projects/*.md` beside the WaveChat surface. In the wave
list, failed waves sort above waiting, running, and idle rows.

Validation run during gate:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
cd website && uv run python dev.py test
swift test --package-path swift -Xswiftc -gnone
tests/e2e/test_smoke.sh
cd swift && xcodegen generate
```

The CI-equivalent Concerto command needed one `xcodebuild clean` after stale
DerivedData blocked writing the UI-test binary, then built the app and passed 5
XCTest tests plus 304 Swift Testing tests. The UI runner still failed before
bootstrapping XCTest in this headless run:

```bash
xcodebuild clean -project LoopflowSwift.xcodeproj -scheme Concerto
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Failure text: `ConcertoUITests-Runner ... Early unexpected exit, operation never
finished bootstrapping ... Test crashed with signal kill before establishing
connection.`

## Intent

Move Concerto's wave viewer onto the new flowloop shape without adding another
backend path. The wave charter now carries only the Objective, projects carry
the measures, and the app renders that local plan next to the existing live
WaveChat frame.

## Assumptions

- Slice 1 is local-file only: `GOAL.md` and `projects/*.md` are available in the
  selected repo checkout.
- Project files use `# Title`, optional summary prose, and a `## KRs` markdown
  list.
- Remote plan loading waits for a future `lf wave show` surface.
- Tasks remain flat in Linear for now, so `Project.tasks` is not surfaced here.

## Key decisions

- Added `WavePlan` / `WaveProject` as frontend domain state, not wire DTOs.
- Parsed the plan for live registry rows and authored-on-disk placeholder rows.
- Sorted the repo wave list by attention priority: failed before waiting before
  running before idle.
- Kept WaveChat as the live interaction surface; Concerto frames the plan but
  still does not render vendor assistant turns.
- Restarted `wave/concerto/GOAL.md` around the Objective and moved the live KRs
  into one markdown file per project.

## Not included

- Runs ledger, charting, pubsub updates, and attachable session rows.
- Linear task ingestion or per-project task grouping.
- New lfd endpoints or DTO fixture changes.
- Remote plan loading.
