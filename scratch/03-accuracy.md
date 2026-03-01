# 03: Reference Accuracy Pass

## Problem

Sprint 02 wrote docs from tribal knowledge. Every page has claims about commands, APIs, file formats, and features that may not match reality. Cross-page consistency is loose — the same concept described differently in README vs docs vs reference pages. Users hitting wrong commands or phantom features lose trust fast.

This is a docs-only repo. We can verify internal consistency and cross-links. We flag claims that require a live environment.

## Approach

Category-based sweep across all pages. Group claims by type, cross-reference, fix inconsistencies, flag unverifiable claims.

### Categories

**1. Command inventory** — Master list of every `lf`, `lfq`, `lfd` command shown in docs. Verify consistent syntax across pages. Known issues found during research:

| Discrepancy | Where | Fix |
|---|---|---|
| `lf --flow build` | `docs/index.md:149` | Should be `lf build` (release notes: "Simpler flow commands") |
| `lfq show` | `waves.md:129` | Undocumented elsewhere — add to README's lfq section or remove |
| `lfq delete` | `waves.md:171` | Undocumented in README — add or remove |
| `lfq stop` | `waves.md:171` | Not in README's lfq commands list — add |

**2. Python API signatures** — Every `loopflow.*` call across docs. Verify consistent signatures, realistic import paths.

| Discrepancy | Where | Fix |
|---|---|---|
| `loopflow.Stimulus(kind="loop")` | `waves.md:55,71,87,117,125` | Class never imported or explained — document or simplify examples |
| `loopflow.update_wave(..., status="paused")` | `waves.md:135` | `status` parameter undocumented — verify and document or remove |
| Mixed `lfq create` + Python `create_wave` | README vs waves.md | Not wrong but confusing — pick one primary path per page |

**3. Conceptual consistency** — Same concept, same definition, everywhere.

| Discrepancy | Where | Fix |
|---|---|---|
| Wave = "4 primary fields" (incl. Stimulus) | README:11 | Contradicts `waves.md:8`: "Stimuli are separate entities" |
| Flows described as Python | `config.md:95` says "Flows are stored as Python files" | Wrong — flows are YAML in `.lf/flows/`. Fix to show YAML. |
| Stimulus list: "Watch, loop, cron, or listen" | README:18 | Missing "Once" — but `docs/index.md:99` and `waves.md` include it |

**4. Cross-links and navigation** — ~~Every `[Link](page.md)` resolves. `_config.yml` has all pages.~~ **VERIFIED: all clear.** All markdown links resolve, all footer/See Also links valid, Journey table links valid, `_config.yml` complete. No work needed.

**5. Terminology** — ~~"sprint" vs "item" vs "stage".~~ **VERIFIED: already clean.** "Sprint" doesn't appear in any user-facing docs (docs/ or README.md). Only in internal wave/step files where it's fine. No work needed.

**6. Forward-looking claims** — Features from the "don't document yet" list that appear in docs.

| Feature | In docs? | Action |
|---|---|---|
| `lfq usage` / token analytics | README:214-215 | Keep — shipped in v0.9.5 (`cost: add billing view...` #515) |
| `lfq providers` | README:216 | Keep — shipped in v0.9.5 |
| Direction aliases | Not found | Good — not documented |
| Cross-repo area resolution | Not found | Good — not documented |
| Sandbox executor details | Not found | Good — not documented |
| Voice input | Not found | Good — not documented |
| Hosted SaaS | Not found | Good — not documented |

**7. Port number** — `lfd.md` hardcodes `127.0.0.1:2486` in 15+ places. Default is 2486, but docs should encourage using `LFD_HTTP_ADDR` with a fresh port (multiple daemons on one machine shouldn't fight over the same socket).

- Remove specific port from intro sentence (line 8). The intro should describe what lfd does, not its socket address.
- Keep `LFD_HTTP_ADDR` env var docs (line 176) with default 2486, but note that picking a unique port per daemon is encouraged.
- curl examples: use a shell variable (`$LFD_ADDR`) so readers set it once. Document the default once at the top of the API section.
- No port references exist outside lfd.md — other docs are clean.

### Execution order

1. **README wave definition rewrite** — Shrink table to 3 fields. Add use-case-framed stimulus section (scheduled vs reactive, with example workflows).
2. **Command syntax fixes** — `lf --flow build` → `lf build` in index.md. Add `lfq show`, `lfq stop`, `lfq delete` to README's lfq section. Fix README stimulus list to include "once."
3. **Python API cleanup** — Simplify `loopflow.Stimulus(kind=...)` pattern in waves.md (class is never imported/explained). Verify `status="paused"` parameter usage.
4. **Fix flow format in config.md** — config.md:95 incorrectly shows flows as Python files. Flows are YAML in `.lf/flows/`. Rewrite to match index.md's correct YAML representation.
5. **Port cleanup in lfd.md** — Remove port from intro sentence. Add `LFD_ADDR` variable note at top of API section. Flag default port for live verification. Stop hardcoding across 15 curl examples.
6. ~~Cross-links~~ — Already verified, all clear.
7. ~~Terminology~~ — Already verified, no "sprint" in user-facing docs.
8. ~~Forward-looking claims~~ — Already verified, `lfq usage` and `lfq providers` correctly present, everything else correctly absent.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| RLM sub-agent audit per page | Parallel but produces redundant findings per page; synthesis overhead | Category-based catches cross-page issues directly |
| Source code verification | Would verify every claim against implementation | This is a docs-only repo; source isn't available here. Flag for live verification instead. |
| Minimal pass (links only) | Fast but misses conceptual drift | The real damage is conceptual inconsistency, not broken links |

## Key decisions

1. **"Item" is the canonical term.** Not sprint, not stage-item. "Items" are numbered markdown files. "Stages" are groups of items sharing a numeric prefix. This aligns with wave-authoring.md which already uses this terminology well.

2. **Wave definition: area × direction × flow.** Stimuli are separate. README table shrinks from 4 rows to 3. Stimulus moves to a new section framed around use cases, not schema:

   - **Scheduled** (proactive): loop continuously, run on cron, run once
   - **Reactive** (triggered): watch main for changes, listen to other waves, auto-fix CI failures

   Lead with example workflows ("loop through a backlog," "rebuild when main moves," "fix CI when it breaks"), not field definitions. The current "Watch, loop, cron, or listen" enum reads like a database column — rewrite around what users are trying to do.

3. **Add `lfq show`, `lfq stop`, `lfq delete` to README.** They appear in realistic contexts in `waves.md` and fit the wave lifecycle management pattern. Omitting real commands from README is worse than documenting phantom ones.

4. **Keep `lfq usage` and `lfq providers`.** They shipped in v0.9.5. The "don't document yet" list is stale on these two points.

5. **`lf build` not `lf --flow build`.** Release notes confirm simpler flow commands shipped. Fix index.md.

6. **Flows are YAML.** `config.md:95` incorrectly says "Flows are stored as Python files" with a Python example. Flows live in `.lf/flows/` as YAML. Fix config.md to show YAML, matching index.md.

7. **Clean up port handling.** Default is 2486 but docs should encourage unique ports per daemon via `LFD_HTTP_ADDR`. Remove port from intro sentence. curl examples use `$LFD_ADDR` variable with one-time setup note. Stop hardcoding across 15 URLs.

## Scope

- **In scope:** README.md, all 9 docs pages, `_config.yml`
- **In scope:** Fixing internal consistency, cross-links, terminology, stale command syntax
- **In scope:** Flagging claims that require live verification
- **Out of scope:** Running `lf`, `lfq`, `lfd` to verify behavior
- **Out of scope:** New docs pages or sections
- **Out of scope:** Concerto-specific documentation

## Done when

- README wave table is 3 fields (area, direction, flow) with stimulus in a separate use-case-framed section
- Stimulus section teaches scheduled (loop, cron, once) vs reactive (watch, listen, ci-fix) with example workflows
- `lf --flow build` → `lf build` everywhere
- `lfq show`, `lfq stop`, `lfq delete` in README's lfq section
- Python API examples use consistent, explained signatures (no unexplained `Stimulus` class)
- config.md flow section fixed to YAML (was incorrectly showing Python)
- lfd.md port references cleaned up: no port in intro, curl examples use `$LFD_ADDR`, unique ports encouraged
- ~~Cross-links~~ already verified clean
- ~~Terminology~~ already verified clean
- ~~Forward-looking claims~~ already verified clean

**Wave goals advanced:** "Every agent integration is accurately documented" and "README, docs site, and in-app guidance tell the same story."
