# 03: Reference Accuracy

**Finish line:** Every claim in README and docs is verifiable — no dead links, no phantom features, no wrong commands.

## Problem

Sprint 02 wrote docs from tribal knowledge in the prompt files, not from running code. Command syntax, API signatures, wave directory conventions, and lfq commands all need verification against the actual implementation. The review explicitly deferred this.

## What to verify

Reference pass across all docs pages. For each claim: can you run it, find it, or link to it?

- Command syntax (`lf`, `lfq`, `lfd` commands shown in docs)
- Python API signatures (`loopflow.create_wave(...)`, `loopflow.run_wave(...)`, etc.)
- Wave directory conventions (README sections, YAML fields, numbered item format)
- Agent compatibility matrix (what works, what's experimental)
- Internal links between docs pages (no 404s)
- `_config.yml` header_pages — `waves.md` is reachable via cross-links but missing from site nav (pre-existing gap, fix here)

## Forward-looking awareness

13 active waves are changing the product. Know what's stable vs evolving:

**Safe to document fully (stable):**
- Step/Flow/Direction/Area model (core primitives)
- Wave directory structure (README + yaml + numbered items)
- Stimulus types: once, loop, watch, cron
- `lfq` commands (create, run, list, logs, stop)
- Auto-loop cycle (ingest → kickoff → build → update-wave)
- tmux plugin (ships independently via TPM)

**Document carefully (evolving):**
- Listen stimulus — Chords wave reworking inter-wave coordination. Verify current behavior, don't over-explain signal architecture.
- Auth — OAuth broker and API key fallback shipping. Verify `lfq auth` commands work as documented.
- Remote lfd — Studio auth, discovery, hosted SaaS in flight. Keep setup specifics light.

**Don't document yet:**
- Direction aliases (Context wave)
- Cross-repo area resolution (Cross-Repo wave)
- Concerto context UI / cost analytics (Context + Cost waves)
- Sandbox executor details (Sandboxes wave)
- Voice input (Voice Control wave)
- `lfq usage` / token analytics (Cost wave)
- Hosted SaaS (Remote wave item 06)

## Terminology

`.lf/steps/` prompts use "sprint"; builtin copies use "item"/"stage"/"roadmap." Not introduced by this wave, but the accuracy pass will surface it. Consider consolidating during this sprint if a clear winner emerges.

## Scope

- README.md, docs/getting-started.md, docs/wave-authoring.md, docs/index.md, docs/waves.md
- `_config.yml` header_pages completeness
- Kill every reference to features that don't exist or commands that don't work
- Flag claims that can't be verified without running infrastructure (lfd, Concerto) — mark as "requires live environment" rather than guessing
