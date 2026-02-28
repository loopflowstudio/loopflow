# Docs & Onboarding

## Vision

Loopflow's docs and onboarding become the best coding-agent setup experience available. A new user on any platform runs `lf init`, gets a working config in 30 seconds, and understands the three ways to use loopflow: raw CLI for learning, Concerto for visual development, remote lfd for serious swarms. The docs teach waves as the core concept and decompose into steps/flows/directions as reference material. Wave authoring — sprints, goals, backlogs, auto-loop — is a first-class documented workflow.

### Not here

- Gemini CLI support (harness doesn't exist yet — add to docs when it ships)
- Concerto feature development (this wave is docs and onboarding, not app features)
- New steps or flows (document what exists, don't build new ones)

## Strategy

Today's docs and onboarding have three problems:

1. **Init is broken on Linux.** It hard-gates on macOS/Homebrew, doesn't know OpenCode exists, and tries to install things instead of detecting them. Half of potential users bounce at the front door.

2. **Docs teach tools, not workflows.** The current structure is "here are steps, here are flows, here are waves" — bottom-up component reference. Users need top-down workflow guidance: "here's what you're trying to do, here's how." The three workflows (lf raw, Concerto, remote lfd) aren't articulated anywhere.

3. **Wave authoring is undocumented.** Waves auto-loop through ship → ship-roadmap. They have sprints, goals, backlogs, automatic triggers like ci-fix. None of this is written down. Users who want autonomous agents have no guide for feeding them work.

The fix: detection-only setup that works on any platform, docs restructured around workflows instead of components, and end-to-end wave authoring documentation.

## Goals

- `lf init` works on macOS and Linux with zero platform-specific dependencies
- New users understand within 5 minutes which mode fits them (lf raw, Concerto, remote lfd)
- A developer can author a wave from scratch using only the docs — vision, sprints, goals, triggers, auto-loop
- Every agent integration is accurately documented (what works, what's experimental, what's planned)
- Setup entry points (lf init, lfd install, Concerto) have clear ownership and hand off cleanly
- README, docs site, and in-app guidance tell the same story

## Risks

- **Init prompt is LLM-executed.** It's a markdown prompt, not compiled code. Cross-platform detection logic in a prompt is fragile — the agent might guess wrong about package managers. Mitigate by keeping init focused on detection (which commands exist) not installation (running package managers).
- **Docs restructure touches 12+ pages.** Risk of inconsistency during transition. Mitigate by doing architecture (nav, structure, new pages) in sprint 2, accuracy (content verification) in sprint 3.
- **Wave content model is still evolving.** Auto-loop, ship-roadmap, ci-fix triggers — some of this is new. Document what's stable, flag what's experimental.
- **Three-workflow framing might not survive contact with users.** The lf-raw / Concerto / remote-lfd split is clean but might not match how people actually discover loopflow. Validate by writing the getting-started page and seeing if it flows.
- **Prompt terminology divergence.** `.lf/steps/` uses "sprint"; builtin copies use "item"/"stage"/"roadmap." Not introduced by this wave, but documenting prompts in sprint 02/03 will surface it. Consider consolidating terminology during the accuracy pass.

## Metrics

- `lf init` success rate across platforms: % of attempts that complete without error (target: >95% on macOS + Ubuntu + Arch)
- Time from `lf init` to first successful step run: minutes (target: <5min for a new user)
- Number of stale/inaccurate doc references found per audit (target: 0 after accuracy pass)
- Page count per workflow (lf raw, Concerto, remote lfd): track completeness (target: 1 dedicated page each)
