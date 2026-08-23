# Consistent Architecture Drift Evidence

## Problem

`scripts/check_architecture.py` currently defines reviewed text by recursively
walking `SCAN_ROOTS`, then subtracting `IGNORED_PARTS` and
`IGNORED_PREFIXES`. Git independently defines which files belong to the review.
Those boundaries have diverged: on 2026-08-23 the normal Loopflow checkout maps
all 167 discovered architecture boundaries but reports 43 stale-vocabulary
errors from ignored archives under `.lf/prompts/` and `.lf/tmp/`; the same
revision is green in a clean hosted checkout because those archives are absent.

This makes the Technical Architecture endurance signal depend on checkout
history instead of reviewed source. Maintainers cannot trust a local red result,
and hosted green runs do not reproduce the tree that appeared to fail.

## The demo

Run `uv run python scripts/check_architecture.py --json` in a long-lived
Loopflow checkout containing ignored prompt and runtime archives. It returns
`"ok": true` with the same eight categories and 167/167 coverage as the clean
hosted checkout; putting the same retired term in tracked `.lf/config.yaml`
makes the command fail with that file and line.

## Approach

Make Git's review candidate set the sole content boundary for the two
repository-wide scans. `_scan_files()` will ask `git ls-files` for cached files
plus non-ignored untracked files under `SCAN_ROOTS`, using standard Git excludes
and NUL-delimited output. It will then retain the existing `TEXT_SUFFIXES` and
read current working-tree bytes.

Concretely, the enumerator will use the semantic equivalent of:

```text
git -C <root> ls-files --cached --others --exclude-standard -z -- <SCAN_ROOTS...>
```

Tracked files remain in scope even if an ignore rule also matches them.
Modified tracked files are checked before commit. Non-ignored new files remain
in scope before staging, preserving the checker's current ability to catch a
new migration, command source, or compatibility marker during development.
Ignored and repository-excluded files are absent because Git, not the
architecture checker, owns that classification.

Delete `IGNORED_PARTS` and `IGNORED_PREFIXES`. Do not replace them with `.lf`
exceptions or retired-term exceptions. Existing `.gitignore` rules already own
the observed runtime directories and the generated `website/docs/` copy. If a
future generator leaks review noise, its output belongs in Git's ignore policy;
the checker must not grow a second policy.

`_discover_shims()` and `_vocabulary_errors()` will continue to share
`_scan_files()`, so compatibility seams and vocabulary receive exactly the same
boundary. The other six discovery categories keep their existing explicit
sources and algorithms.

Failure to enumerate the Git tree will be a clear checker error, never an empty
or filesystem-walk fallback. A fallback would restore environment-dependent
evidence precisely when the authority is unavailable.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Is the reported failure really content-boundary drift rather than missing architecture coverage? | The archive-heavy checkout reports 47/47 public APIs, 8/8 process boundaries, 50/50 SQLite owners/mirrors, 6/6 projections, 18/18 routes, 6/6 providers, 26/26 subprocess edges, and 6/6 shims. Its only failures are 43 retired-term matches under ignored `.lf/prompts/` and `.lf/tmp/`. | Change only shared repository-wide file enumeration; do not weaken a category or vocabulary rule. |
| Can Git express the desired set without parsing ignore files? | `git ls-files --cached --others --exclude-standard` combines tracked paths with non-ignored untracked paths and applies repository, info, and configured standard exclusions. `-z` makes filename parsing unambiguous. | Use Git directly as the authority; add no ignore parser or dependency. |
| Does this preserve reviewed `.lf` configuration? | `.lf/config.yaml`, `.lf/flows/`, `.lf/directions/`, and `.lf/skills/` are tracked and returned by the Git query. `.lf/prompts/` and `.lf/tmp/` are excluded by existing `.gitignore` rules. | Keep `.lf` in `SCAN_ROOTS`; remove only the checker's private exclusion policy. |
| Does the candidate survive the real counterexample? | A local prototype using the Git enumerator against the archive-heavy checkout returned `ok: true` with the same 167/167 coverage. It selected 574 review-candidate paths, 540 with scanned text suffixes, and none under `.lf/prompts/` or `.lf/tmp/`. | The chosen mechanism resolves the observed local/hosted split without term knowledge. |
| Will hosted and weekly evidence take the same path? | `.github/workflows/ci.yml` and `.github/workflows/architecture-drift.yml` both invoke `uv run python scripts/check_architecture.py` directly; only the weekly job adds `--json`. | No workflow mode, environment flag, or duplicate wrapper is needed. |
| Could the test fixture accidentally stop covering new files? | The present fixture is not a Git repository. Initializing it and staging the baseline lets existing mutation tests model tracked edits, while `--others` keeps tests that create new source files meaningful before staging. | Convert the fixture into a temporary Git repository rather than mocking Git or exposing a test-only file provider. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Add `.lf/prompts` and `.lf/tmp` to `IGNORED_PARTS` or `IGNORED_PREFIXES` | Smallest immediate diff. | It duplicates `.gitignore`, fixes only today's archive names, and guarantees the next generated directory can split local and hosted evidence again. |
| Scan only `git ls-files --cached` | Gives the strict tracked path set and excludes every untracked local file. | It misses a newly created, non-ignored source file until staging, weakening the existing local pre-commit signal for new migrations, shims, and source roots. |
| Keep the filesystem walk and implement `.gitignore` matching in Python | Avoids invoking Git. | Correct Git ignore semantics include nested rules, negation, repository excludes, configured global excludes, and tracked files that match later ignore rules. Reimplementing them creates another authority and is harder than the original task. |

## Key decisions

- Define a review candidate as a tracked path or a non-ignored untracked path.
  This is intentionally broader than HEAD and narrower than the filesystem.
- Keep `SCAN_ROOTS` as the bounded architecture scope and `TEXT_SUFFIXES` as
  the supported source formats. They answer what architecture evidence covers;
  Git answers whether a path belongs to the review.
- Read working-tree bytes for returned paths. The checker must catch unstaged
  edits to tracked files, not merely judge the index blob.
- Preserve all eight coverage algorithms and exact retired-term matching.
- Make Git enumeration failure visible. Do not silently fall back, skip the
  scan, or report zero discovered evidence.
- Keep this as one coherent PR. The production boundary and its behavioral
  proof are not independently useful.

Wild success is boring: old prompt archives can accumulate indefinitely while
local, PR, and weekly runs agree; maintainers stop inspecting false positives,
and any real stale term still names a reviewed file and line. Wild failure is a
quietly narrower scan—tracked edits or new source disappear, Git failure becomes
green, or another private exclusion list begins growing. The tests and explicit
failure behavior target those failure modes.

## Scope

- In scope: Git-backed enumeration for the shared shim/vocabulary scan;
  deletion of the checker's manual filesystem exclusions; Git-realistic
  architecture test fixtures; behavioral proof for ignored runtime archives
  and reviewed `.lf` vocabulary; focused and real-checker verification.
- Out of scope: changing architecture categories, map contents, retired terms,
  historical allowed scopes, `.lf` runtime retention, workflow schedules, KR
  status, or a generic repository-file abstraction.
- Out of scope: marking any 30-day or four-week endurance KR complete from this
  single repaired run.

## Done when

- A focused test stages the fixture baseline, adds ignored `.lf/prompts/` and
  `.lf/tmp/` text containing retired vocabulary (and a stray shim marker), and
  proves the complete report is unchanged.
- A focused test puts the equivalent retired vocabulary in tracked
  `.lf/config.yaml` and proves the checker reports its exact path and line.
- Existing tests still prove that non-ignored new migrations, source, and shim
  files are discovered before staging.
- `uv run pytest python/tests/test_architecture.py` passes.
- `uv run python scripts/check_architecture.py --json` returns `ok: true` and
  preserves all eight categories at 167/167 in the current worktree.
- The updated enumerator run against the archive-heavy normal checkout also
  returns `ok: true` at 167/167, closing the recorded counterexample.
- The hosted architecture check uses the same script and passes without a
  local/CI flag or workflow-specific exclusion.

## Forbidden outcomes

- A stale-term, `.lf/prompts`, `.lf/tmp`, timestamp, archive-name, or generated
  path allowlist in the checker.
- Scanning HEAD blobs while ignoring current tracked edits.
- Excluding every untracked file and losing pre-stage detection of new reviewed
  candidates.
- Dropping reviewed `.lf` configuration, skills, directions, or flows.
- A Git-failure fallback that walks the filesystem or returns an empty scan.
- A green total achieved by removing a category, reducing its discovered set,
  or weakening exact vocabulary matching.
- Different command paths or environment switches for local and hosted runs.

## Internal slices

1. Replace `_scan_files()` with NUL-safe Git candidate enumeration and delete
   the redundant ignore constants. Surface enumeration failure as a report
   error through the existing checker error path.
2. Initialize and stage the temporary test repository, then add the ignored
   archive invariance and tracked `.lf` failure proofs. Keep existing new-file
   behavior tests green through `--others` coverage.
3. Run the focused suite, the real checker, and the archive-heavy checkout
   probe. Confirm workflow commands remain the same and record exact evidence
   in the slice ledger.

## This slice

Implement all three internal slices in this PR. The focused proof is
`uv run pytest python/tests/test_architecture.py`; the defining end-to-end proof
is identical 167/167 output from the clean task worktree and the normal checkout
that currently contains ignored runtime archives.

## Slice ledger

- 2026-08-23 kickoff baseline: the task worktree is green at 167/167 because it
  has no ignored prompt/tmp archive population.
- 2026-08-23 counterexample reproduced: the normal checkout maps 167/167 but
  fails on 43 stale-vocabulary occurrences, all under ignored `.lf/prompts/`
  and `.lf/tmp/`.
- 2026-08-23 candidate probe: Git enumeration selects 540 scanned text files,
  excludes both runtime archive trees, preserves all eight category totals, and
  makes the normal checkout green at 167/167.
- Implementation, test, CI, and review evidence remain to be appended.

