# Wave Restructure

Consolidate 13 waves into 5 competency tracks. Each wave represents a core competency that gets better forever, not a feature backlog that empties.

## New Waves

### Scale
**Coordinate many agents across repos and workflows.**

Cross-repo portfolios, chords, FlowRun container, stimulus coordination, branch routing. Everything about loopflow working at org scale rather than one-task-at-a-time.

### Foundation
**The system works correctly.**

Test coverage, code cleanup, API completeness, operational validation. Making sure what exists is solid before building more.

### Trust
**Safe to leave running overnight.**

Security hardening, credential encryption, studio auth, container isolation, sandboxes. You hand loopflow your credentials and walk away.

### Context
**Agents see the right things at the right time.**

Direction aliases, doc inclusion policy, token budgets, prompt transparency. The prompt pipeline — loopflow's deepest competitive advantage.

### Concerto
**The app humans use to steer agents.**

Mobile experience, voice input, interaction patterns, app-level polish. Vertical feature UI (chords viz, cross-repo portfolio, context breakdown) lives with its domain wave, not here.

---

## Sprint Ordering

Bias towards learning. Put the sprint that teaches you the most first — later sprints benefit from contact with reality.

### Scale

| # | Sprint | Why this order |
|---|--------|---------------|
| 01 | flowrun-container | Foundational. Everything else assumes this data model exists. Riskiest work — do it first and learn what breaks. |
| 02 | cross-repo-commits | Lowest-level cross-repo primitive. Teaches how multi-repo detection actually works before adding coordination on top. |
| 03 | cross-repo-stimulus | Builds on commits. Adds the reactive coordination layer — now you know what "listen across repos" means concretely. |
| 04 | chords-ui | Visualizing orchestration. Needs FlowRun to exist. By now you understand the execution model well enough to show it. |
| 05 | cross-repo-ui | Portfolio view. Last because it's the widest surface and benefits from all the backend learning. |

### Foundation

| # | Sprint | Why this order |
|---|--------|---------------|
| 01 | daemon-test-coverage | Map the gaps first. Writing tests teaches you where the bodies are buried. |
| 02 | code-cleanup | Easier after tests — you know what's dead, what's duplicated, what's misplaced. |
| 03 | mac-mini-dogfood | Operational validation on real hardware. Tests and cleanup make this more productive — you're not fighting known issues. |
| 04 | api-expansion | Remote-safe APIs for file browsing and metadata. Builds on what dogfooding revealed about what's missing. |
| 05 | container-hardening | Native fallback docs, container boundary clarity. Informed by everything above. |

### Trust

| # | Sprint | Why this order |
|---|--------|---------------|
| 01 | security-hardening | Credential redaction, non-root container, Sendable audit. Broad survey — teaches where the security gaps are. |
| 02 | credential-encryption | Encryption at rest. Design decisions clearer after hardening reveals the full credential surface. |
| 03 | studio-auth | JWT validation, studio identity, Concerto sign-in. Builds on understanding the credential model. |
| 04 | sandbox-integration | Blocked on DinD. Pick up when unblocked. |
| 05 | sandbox-rollout | Blocked. Depends on integration validation. |

### Context

| # | Sprint | Why this order |
|---|--------|---------------|
| 01 | direction-aliases | Teaches the resolution pipeline end-to-end. Small surface, high learning. |
| 02 | context-ui | Visualization. Needs understanding of the data flowing through the pipeline. |

### Concerto

| # | Sprint | Why this order |
|---|--------|---------------|
| 01 | queue-management | Small, teaches the reply/input flow. Low risk, immediate learning about the Swift codebase. |
| 02 | api-key-entry | Small, teaches Connection Settings surface. Thin UI over existing endpoint. |
| 03 | release-ui | Release config + "Release Now" button with version picker. Builds on app understanding. |
| 04 | auto-send | Most complex. Voice pipeline, confidence scoring, continuous mode. Benefits from confidence in the codebase. |

### Kill

| Old wave | Sprint/Wave | Reason |
|----------|-------------|--------|
| prune | 02-prune-step | Small chore, not wave-worthy. Do it when it bothers you. |
| release-minor | (whole wave) | Working cron config. Trivial to recreate. |
| release-patch | (whole wave) | Working cron config. Trivial to recreate. |
| remote | 06-hosted | Far out, no near-term sprints. Add back when it's real. |

### Waves dissolved

All 13 old waves cease to exist. Their sprints migrate:

- **auth** → Concerto (API key entry), Trust (credential encryption)
- **chords** → Scale
- **cross-repo** → Scale
- **infra** → Foundation (tests, cleanup), Trust (security hardening)
- **mobile** → Concerto
- **opsflows** → Concerto (release UI)
- **prune** → killed
- **release-minor** → killed
- **release-patch** → killed
- **remote** → Foundation (dogfood, API expansion, container hardening), Trust (studio auth), hosted killed
- **sandboxes** → Trust
- **voicecontrol** → Concerto

---

## Done when

- `wave/` contains 5 directories: scale, foundation, trust, context, concerto
- Each has a README with vision framed as a competency track
- Each has a YAML config
- Each has numbered sprint files migrated from old waves (content preserved, renumbered 01-XX)
- Old wave directories deleted
