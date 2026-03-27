# gstack

Import Garry Tan's gstack prompt sequence as loopflow flows and steps, with sync tooling to stay current with the upstream repo.

## Vision

Loopflow becomes a platform for workstyles — shareable bundles of prompts, flows, voice, and config that define how someone works with coding agents. gstack is the first external workstyle, proving that loopflow can ingest and run prompt sets from other creators while preserving their identity and voice.

Three launch workstyles: **lfjack** (loopflow native), **vsm** (governance), **gstack** (sprint factory). Eventually: **loopflowhub** — an open-source registry where anyone publishes their workstyle.

**Sync as normalizer.** People publish in whatever form they want — SKILL.md files, a single program.md, raw markdown, yaml configs. The sync engine reads any input format and writes directly to loopflow's native formats: `.md` steps, `.yaml` flows, `voice.md`, `config.yaml`. No intermediate representation — it writes to the places loopflow already looks. Missing pieces get lfjack defaults.

### Not here

- loopflowhub registry infrastructure
- vsm and lfjack workstyle packaging (separate waves)
- gstack's browser daemon (`/browse`, `/qa` browser automation)
- gstack's cross-agent setup (codex/gemini compatibility layer)

## Goals

- Run any gstack prompt as a loopflow step: `lf gstack:office-hours`
- Run the gstack sprint as a loopflow flow: `lf gstack-sprint`
- Sync from garrytan/gstack with one command: `lf ops workstyle sync gstack`
- Preserve gstack's voice — the workstyle carries its personality
- Existing loopflow steps (implement, gate) compose cleanly inside gstack flows

## Risks

- gstack's SKILL.md format has substantial preamble machinery (telemetry, update checks, session tracking). The converter must strip this cleanly without losing prompt content.
- gstack moves fast (v0.12 in two weeks from creation). Sync tooling needs to handle format changes gracefully.
- Some gstack skills depend on the browser daemon — those won't work without it. Need clear error messages.
- Voice priority: when gstack voice and loopflow voice conflict, gstack should win during gstack flows. This requires runtime voice resolution changes.

## Metrics

- Number of gstack SKILL.md files successfully converted (target: all 28)
- Time to sync from upstream (target: <30s)
- Zero manual edits needed after sync to run gstack flows
