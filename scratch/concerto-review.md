# Concerto gate review

## What was implemented

Concerto's wave detail pane now renders the new flowloop wave plan beside the
live WaveChat surface. The plan is loaded from local files:
`wave/<name>/GOAL.md` provides the Objective, and `wave/<name>/projects/*.md`
provides live projects with summaries and KRs.

The Swift model adds `WavePlan` and `WaveProject`, stores a parsed plan on
`WaveViewModel`, and attaches that plan to both live lfd/registry rows and
authored-on-disk placeholder rows. The Concerto wave charter was also restarted
onto the objective-only GOAL.md shape, with five live project files carrying the
measures that used to live in the charter.

## Key choices

- Keep slice 1 local and file-first. The parser reads the wave directory
  directly instead of adding new lfd or `lf wave show` wire shape.
- Model projects now, leave tasks empty. Linear task grouping is not resolved
  yet, so this branch does not fake per-project tasks.
- Show the plan only when local content exists. Remote/iOS and waves without
  authored files fall back to the existing WaveChat-only detail.
- Preserve the old `WaveContent` path for legacy row taglines. The new plan view
  does not remove that older parser yet because the current row UI still uses it.

## How it fits together

`WavePlanParser` maps `GOAL.md` and `projects/*.md` into `WavePlan`.
`PortfolioRepoState`, authored placeholders in `WavesView`, and `RepoState`
attach that plan to `WaveViewModel`. `WaveDetailPane` then splits the detail
surface: plan on the left, live WaveChat on the right.

## Risks and bottlenecks

- The parser intentionally recognizes the committed file shape: exact
  `## Objective` and `## KRs` headings with markdown list items. If the project
  file grammar changes, update parser tests first.
- Local file reads are synchronous and small today. If wave projects grow large
  or move remote, this should become an `lf wave show` query surface.
- The future runs ledger is not included. `scratch/questions.md` records run
  status and origin-shape gaps found during review before slice 2 hardens.
- Full Concerto UI validation could not complete in this headless run:
  `ConcertoUITests-Runner` was killed before XCTest bootstrapped. The app built
  and 309 tests passed before that runner-level failure.

## What's not included

- No runs ledger, chart, history grouping, or live pubsub wiring.
- No Linear task loading or project-to-task assignment.
- No new lfd endpoint or DTO change.
- No remote plan loading; remote detail still falls back to WaveChat when local
  files are unavailable.

## Validation

- `swift test --package-path swift -Xswiftc -gnone --filter WavePlanParserTests`
  passed: 3 tests.
- `uv run python scripts/test.py` passed changed-aware validation: Swift package
  suite, 304 tests.
- `cd swift && xcodegen generate` passed.
- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  built and ran 309 passing tests, then failed because `ConcertoUITests-Runner`
  exited early before bootstrapping XCTest in the headless environment.
- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo test --all` passed.
- `uv run pytest python/tests/` passed: 54 tests.
- `cd website && uv run python dev.py test` passed: 61 passed, 3 skipped.
- `tests/e2e/test_smoke.sh` passed.
