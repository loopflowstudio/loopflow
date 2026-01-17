# lfdocs

Refresh loopflow documentation to reflect current capabilities and create compelling demos.

## What to build

Updated docs with two recorded demos showing real workflows: quick debugging and full feature development.

## Current State

**Marketing site** (localhost:5001):
- "Arrange agents to code in harmony"
- Three tiers: CLI (stable), Maestro (early), Agents (experimental)
- Core pitch: "Prompts in your repo. Quality gates between steps. Open source."

**Docs** (docs/*.md):
- Written for earlier version
- demo.gif just shows `--help` output
- Missing: `lfwork`, summarization, racing, image context
- Missing: clear "getting started" flows

**Key tasks** that exist now:
- `debug` - Fix error from clipboard
- `design` - Create implementation spec
- `implement` - Build from design doc
- `polish` - Fix issues, run tests
- `review` - Assess and produce verdict

## Demos to Create

### Demo 1: Quick Debug (`lf debug -v`)

Show the "unblock fast" use case. Copy an error, run one command, watch it fix.

**Setup:** A demo directory with a broken Python file:
```
demos/
  debug-demo/
    calculator.py  # has a bug
    test_calc.py   # test that fails
```

**Flow:**
1. Run test, see error
2. Copy stacktrace to clipboard
3. `lf debug -v`
4. Watch fix happen
5. Run test again, passes

### Demo 2: Full Workflow (`lf design` → ship)

Show the "build a feature" use case. Interactive design, then autonomous implementation.

**Setup:** Same demo directory, now building a new feature.

**Flow:**
1. `wt switch --create add-divide`
2. `lf design: add division to calculator`
3. (Interactive: discuss, produce .design/add-divide.md)
4. `lf implement && lf polish && lf review`
5. See working code, tests pass, verdict: ready to ship

### Demo 3: Just `lf` (optional)

Show inline prompts for quick tasks:
```bash
lf : "fix the typo in README"
```

## Demo Setup

**Separate repo:** `loopflow-studio/loopflow-demos` (private). Users clone it to try loopflow.

**Structure:**
```
loopflow-demos/
  calculator/
    calc.py           # has a bug (off-by-one)
    test_calc.py      # test that fails
    README.md         # explains what to try
  .lf/
    config.yaml       # minimal config
```

**Recording:** VHS tape files in main loopflow repo:
```
docs/
  debug-demo.tape     # records lf debug -v workflow
  design-demo.tape    # records design → ship workflow
  debug-demo.gif      # generated output
  design-demo.gif     # generated output
```

## Doc Updates

### index.md (landing page)
- Replace current "What it is" with the two demos
- Lead with `lf debug -v` (instant value)
- Then `lf design` → ship (full workflow)
- Keep "Why this matters" section but tighten

### patterns.md
- Update examples to match current CLI
- Add `lf debug -v` pattern
- Add summarization pattern
- Add racing pattern

### config.md
- Add `summaries:` section
- Add `work:` section (lfwork integration)
- Update any outdated options

### Remove/consolidate
- vision.md content → fold into index.md or delete
- lfd.md → update for current daemon behavior

## Recording

VHS (vhs.charm.sh) tape files for reproducible recordings. Install via `brew install vhs`.

Generate GIFs with:
```bash
vhs docs/debug-demo.tape
vhs docs/design-demo.tape
```

## Constraints

- Demos must work with current loopflow (no mocking)
- Demo directory should be self-contained, deletable
- Keep demos short (<30 seconds each)
- Real agent output, not staged

## Done when

1. `loopflow-studio/loopflow-demos` repo exists with broken calculator
2. VHS tape files record both workflows
3. docs/index.md leads with the two demo GIFs
4. Other docs updated for current features
5. `uv run pytest tests/` passes
