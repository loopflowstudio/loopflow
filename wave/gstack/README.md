# gstack

Import Garry Tan's gstack prompt sequence as loopflow flows and steps, with sync tooling to stay current with the upstream repo.

## Vision

Loopflow becomes a platform for workstyles — shareable bundles of prompts, flows, and config that define how someone works with coding agents. gstack is the first external workstyle, proving that loopflow can ingest and run prompt sets from other creators while preserving their methodology. Style documents like gstack's voice and OpenClaw's soul become reusable directions.

Three launch workstyles: **lfjack** (loopflow native), **vsm** (governance), **gstack** (sprint factory). Eventually: **loopflowhub** — an open-source registry where anyone publishes their workstyle.

**Sync as normalizer.** People publish in whatever form they want — SKILL.md files, a single program.md, raw markdown, yaml configs. The sync engine reads any input format and writes directly to loopflow's native formats: `.md` steps, `.yaml` flows, and config. Style docs can also be extracted into directions when that proves useful. No intermediate representation — it writes to the places loopflow already looks. Missing pieces get lfjack defaults.

### Not here

- loopflowhub registry infrastructure
- vsm and lfjack workstyle packaging (separate waves)
- gstack's browser daemon (`/browse`, `/qa` browser automation)
- gstack's cross-agent setup (codex/gemini compatibility layer)

## Goals

- Run any gstack prompt as a loopflow step: `lf gstack:office-hours`
- Run the gstack sprint as a loopflow flow: `lf gstack-sprint`
- Sync from garrytan/gstack with one command: `lf op gstack sync`
- Preserve gstack's style as a reusable direction
- Make OpenClaw's `SOUL.md` available as a reusable direction too
- Existing loopflow steps (implement, gate) compose cleanly inside gstack flows

## Risks

- gstack's SKILL.md format has substantial preamble machinery (telemetry, update checks, session tracking). The converter must strip this cleanly without losing prompt content.
- gstack moves fast (v0.12 in two weeks from creation). Sync tooling needs to handle format changes gracefully.
- Some gstack skills depend on the browser daemon — `browse`, `qa`, `connect-chrome`, `setup-browser-cookies` won't work without it. Imported steps now reference `.lf/steps/gstack/` paths but the actual binaries and helper scripts aren't imported. Need separate asset packaging or clear runtime errors.
- Converting style docs into directions may reveal that "voice" is too narrow a concept. The imported document may need reshaping to fit loopflow's direction model cleanly.
- The converter performs opinionated reference rewriting and telemetry stripping. Upstream format changes may break these rules — sync tooling needs to handle this gracefully.

## Metrics

- Number of gstack SKILL.md files successfully converted (target: all 29)
- Time to sync from upstream (target: <30s)
- Zero manual edits needed after sync to run gstack flows

## Key milestone

Jack runs `lf gstack-sprint` on a real project and ships something with it. The gstack flow isn't done until it's been used end-to-end by an actual user, not just tested in isolation.
