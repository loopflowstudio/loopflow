# 03: Reference Accuracy

## Problem

The docs tell a mostly accurate story but have systematic errors that will trip up anyone who copies a code example and runs it. The worst offender: `docs/waves.md` uses `update_wave(..., stimulus=...)` in 5 code examples, but that parameter doesn't exist — stimuli are managed through `add_stimulus()`. A new user following the wave docs will get `TypeError` on their first attempt.

Beyond the API error, there's a cluster of stale references: flows described as Python files when they're YAML, a planning step (`polish`) presented as a test runner, a phantom `--flow` flag, and inconsistent Gemini documentation that says "supported" in some places and "not here" in others.

**Who benefits:** Every new user who reads the docs and tries the examples. Every existing user who checks the docs to learn a feature they haven't used yet.

**Why now:** Sprints 01 and 02 are shipped. The accuracy pass is the final sprint — cleaning up everything so the docs are trustworthy end-to-end.

## Approach

Systematic fix pass across all 8 doc files plus README.md. Group fixes by severity: wrong API calls first, then wrong descriptions, then stale/misleading references.

### Critical fixes

**1. `docs/waves.md` — wrong stimulus API (5 locations)**

Replace all `loopflow.update_wave(..., stimulus=loopflow.Stimulus(...))` with `loopflow.add_stimulus(...)`.

Lines 55, 71, 87, 117, 125 all use the wrong function. The stimulus table (lines 29-30) also shows the wrong API. Line 102 (listen) correctly uses `add_stimulus` — use that as the model.

Before:
```python
loopflow.update_wave("looper", stimulus=loopflow.Stimulus(kind="loop"))
```

After:
```python
loopflow.add_stimulus("looper", kind="loop")
```

For cron:
```python
loopflow.add_stimulus("cronner", kind="cron", cron="0 9 * * *")
```

For watch:
```python
loopflow.add_stimulus("watcher", kind="watch")
```

The "Multiple Stimuli" section (lines 107-137) needs restructuring. The current example uses `update_wave` to add a second stimulus and `update_wave(..., status="paused")` to disable one. The correct pattern:
- Add stimuli with `add_stimulus()`
- Remove with `remove_stimulus(name, stimulus_id)` (the actual API)
- Pause the wave with `update_wave(name, status="paused")` (this part is correct)

**2. `docs/waves.md` line 19 — wrong flow name in description**

Says "This runs the `ship` flow" but the example creates a wave with `flow="build"`. Fix to say `build`.

### Significant fixes

**3. `docs/config.md` lines 93-101 — flows described as Python**

Says "Flows are stored as Python files in `.lf/flows/`" with a Python `Flow()` constructor example. Flows are YAML files. Replace with:

```yaml
# .lf/flows/my-flow.yaml
- implement
- compress
- gate
```

**4. `docs/index.md` line 77 — flow file extension**

Table says `.lf/flows/*.py`. Change to `.lf/flows/*.yaml`.

**5. `docs/index.md` lines 130-139 — flow invocation and content**

Line 134 shows `build.yaml` as `implement → compress → gate → update-wave` but the actual file is `implement → compress → lint → gate → update-wave` (missing `lint`).

Line 138 shows `lf --flow build`. The `--flow` flag doesn't exist. Flows are invoked as `lf build`. Fix to:

```bash
lf build
```

**6. `docs/getting-started.md` lines 88, 97 — polish misdescribed**

The feature workflow shows `lf polish` after `lf implement` and describes it as "Run tests, fix issues". But `polish` is a planning step that surveys rough edges and writes priorities to `scratch/polish-priorities.md`. It doesn't run tests or fix code.

Two options: (a) replace `polish` with `gate` in the workflow example ("Ship-ready check, run tests, fix issues"), or (b) replace with `lint` ("Run lint and format checks, fix failures"). `gate` is the better fit for a shipping workflow since it runs the full quality check.

Replace the workflow:
```bash
lf design: add OAuth login         # discuss approach
lf implement                       # build it
lf gate                            # ship-ready check
lf ops pr                           # open PR
```

And update the steps table to include `gate` instead of `polish`.

**7. Gemini documentation inconsistency**

- `config.md` line 203: lists `gemini` as a harness — **accurate** for one-shot CLI
- `config.md` line 222: lists `gemini` in `supported_harnesses` example — **accurate** for config
- `troubleshooting.md` line 48-49: mentions Gemini rate limits — **accurate** for CLI use
- `lfd.md` line 142: says supported harnesses are `codex`, `claude`, `opencode` — **accurate** for sessions
- `lf.md` line 79: lists `gemini` as a model option — **accurate** for CLI

The inconsistency isn't wrong per se — Gemini works for `lf` one-shot commands but not for `lfd` sessions/waves. But nowhere does it say this clearly. Add a note to `config.md` in the Model section:

> Gemini is supported for direct `lf` commands. Session-based features (waves, `lfd`) require `claude`, `codex`, or `opencode`.

**8. `docs/lf.md` line 23 — misleading path comment**

`# run .claude/commands/review.md` implies the step file lives at that path. Steps resolve from multiple locations in priority order. Remove the path comment — the preceding lines already explain resolution:

```bash
lf review                    # run the review step
```

### Minor fixes

**9. `docs/lfops.md` line 215 — prune dirty definition**

Says "Never prunes main/master or dirty worktrees." The codebase now uses `is_clean_ignoring_scratch` — leftover `scratch/` files don't block pruning. Update to: "Never prunes main/master or worktrees with uncommitted changes (scratch/ files are excluded)."

**10. README steps table completeness**

The README intentionally shows a curated subset. The table is accurate for what it shows — no step is listed that doesn't exist. The omission of operational steps (`lint`, `commit`, `init`, `rebase`, `land`) and direction steps is a documentation choice, not an error. Leave as-is — the README should stay scannable.

But consider adding `lint` and `gate` to the code steps table since they appear in the `build` flow and the getting-started workflow. These are the steps users will encounter first.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Automated link/API checker | Catches regressions permanently | Good future work but doesn't fix the content errors now. Build this after the manual pass. |
| Rewrite waves.md from scratch | Cleaner result | The structure and prose are good. Only the code examples are wrong. Surgical fixes are less risky. |
| Add Gemini session support to close the doc gap | Eliminates the inconsistency | Wave README says "not here" for Gemini. The gap is in the harness, not the docs. Document reality. |

## Key decisions

**Surgical over structural.** The docs architecture is fine — 8 files covering the right topics in the right order. The problems are specific wrong values in specific lines. Fix those, don't restructure.

**Replace `polish` with `gate` in getting-started.** The getting-started workflow should show steps that actually do what they say. `gate` runs tests, checks quality, and is the step users will actually use in a shipping workflow. `polish` is for planning, not execution.

**Clarify Gemini scope, don't remove it.** Gemini works for `lf` commands. Removing all Gemini references would be inaccurate. Adding a scope note is precise.

**Don't expand the README steps table.** The README is already 200+ lines. Adding every step makes it less useful, not more. The `lf --list` command and `docs/` site exist for completeness.

## Scope

In scope:
- Fix all wrong API calls in `docs/waves.md`
- Fix flow format (Python → YAML) in `docs/config.md` and `docs/index.md`
- Fix `polish` → `gate` in `docs/getting-started.md`
- Fix phantom `--flow` flag in `docs/index.md`
- Fix `build.yaml` content (add missing `lint`) in `docs/index.md`
- Add Gemini scope note to `docs/config.md`
- Fix misleading comment in `docs/lf.md`
- Fix prune dirty definition in `docs/lfops.md`
- Fix flow description mismatch in `docs/waves.md` line 19

Out of scope:
- README restructuring (accurate for what it shows)
- Adding missing steps to README tables
- Automated doc validation tooling
- Gemini session harness implementation
- New documentation pages

## Done when

Every code example in `docs/` is copy-pasteable and produces the expected result:
- `loopflow.add_stimulus()` used for all stimulus operations (not `update_wave`)
- Flow files described as YAML everywhere
- `lf build` invocation (not `lf --flow build`)
- `getting-started.md` workflow uses steps that match their descriptions
- Gemini scope is documented clearly
- `cargo test -p loopflow golden_prompt` passes (if accuracy changes touch prompt files)

Advances wave goals:
- "Every agent integration is accurately documented" — Gemini scope note
- "README, docs site, and in-app guidance tell the same story" — consistent flow format, consistent API usage
- "Number of stale/inaccurate doc references found per audit (target: 0 after accuracy pass)" — this is the pass
