# Review: Reference Accuracy Pass

## What was implemented

Cross-page consistency fixes for README, 5 docs pages, and wave item relocation. Every claim about commands, APIs, file formats, and stimulus types now tells the same story across all pages.

Changes by category:

1. **Wave definition** — Shrunk from 4 fields (incl. stimulus) to 3 (area × direction × flow). Stimulus moved to a new use-case-framed section organized as scheduled vs reactive. Applied consistently in README, index.md, waves.md.

2. **Command syntax** — `lf --flow build` → `lf build` in index.md. Added `lfq show`, `lfq stop`, `lfq delete` to README's lfq section.

3. **Python API** — Replaced `loopflow.Stimulus(kind=...)` with `loopflow.add_stimulus(...)` across waves.md and wave-authoring.md. Removed unexplained `Stimulus` class. Removed `update_wave(..., status="paused")` (undocumented parameter).

4. **Flow format** — config.md incorrectly showed flows as Python files. Fixed to YAML, matching index.md. Updated file extension in index.md atom table (`.py` → `.yaml`).

5. **Port cleanup** — lfd.md intro no longer hardcodes `127.0.0.1:2486`. All curl examples use `$LFD_ADDR` variable. One-time setup block documents the default and encourages unique ports per daemon.

6. **Stimulus coverage** — CiFailure added to waves.md, wave-authoring.md, and index.md's stimulus tables. Listen added to index.md (was missing). All 6 stimulus types now documented consistently across all pages.

## Key choices

- **Stimulus as separate concept, not wave field.** Stimuli configure *when* a wave runs, not *what* it is. The core identity is area × direction × flow. This matches how the API works (`add_stimulus` is a separate call from `create_wave`).

- **Use-case framing over enum listing.** README's stimulus section leads with "scheduled" and "reactive" categories with example workflows, not a flat list of mode names. Teaches intent before mechanics.

- **`$LFD_ADDR` variable pattern.** Set once, use everywhere. Readers running multiple daemons change one line. The default `127.0.0.1:2486` appears exactly where it should: the setup block and the env var reference.

- **Removed One-Shot section from waves.md.** Redundant with the Once subsection directly above it. Same content, different heading.

## How it fits together

The accuracy pass touched 6 docs files with a single theme: every page should agree on terminology, syntax, and API signatures. README is the entry point with the most concise versions. `docs/waves.md` is the reference with full examples. `docs/index.md` and `docs/wave-authoring.md` sit between. All now use the same vocabulary and the same API patterns.

## Risks and bottlenecks

- **No live verification.** This is a docs-only repo. Commands, API signatures, and default ports are verified for internal consistency, not against a running `lfd`. The design doc flags this explicitly.
- **CiFailure is lightly documented.** It gets a table row and one-line description. If it has configuration options (e.g., which step to run, retry behavior), those aren't covered. Sufficient for now since it's an auto-configured default.

## What's not included

- No new docs pages. All changes are fixes to existing pages.
- No Concerto-specific documentation.
- `getting-started.md` was not modified (no inconsistencies found there).
- Source code, tests, and scripts with hardcoded ports are untouched — those are implementation defaults, not user-facing docs.

## Gate fixes applied

- Added Listen to index.md stimulus table and inline description (was inconsistent with all other pages).
- Added CiFailure to waves.md, wave-authoring.md, and index.md (README introduced it but reference pages omitted it).
