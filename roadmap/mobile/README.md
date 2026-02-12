# Mobile Roadmap

iOS app for wave management. Builds on the [remote](../remote/) infrastructure — mobile is a client to remote lfd, same as Concerto on macOS.

## Prerequisite

Remote Phases 03 (Concerto remote connection) and 05 (studio auth) must be working before mobile ships. Mobile needs remote lfd access and real auth — there's no "local lfd on your phone."

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Conduct UI (status + actions) | Todo |
| 2 | Improvise UI (create + trigger) | Todo |
| 3 | Push notifications | Todo |
| 4 | Chat experience | Future |
| 5 | Agent harness | Future |

## Phase 1: Conduct UI

Mobile as remote control. Conductor persona: check in, trigger actions, move on.

- See wave status (running, waiting, idle)
- Land PRs
- Stop/restart waves
- See results and PR links

**Not included:** creating waves, chat, terminal, interactive steps.

## Phase 2: Improvise UI

Create and trigger waves from phone.

- Area picker (touch-adapted)
- Flow picker with typeahead (uses remote/06 prompts API)
- Direction pills
- "Run" triggers non-interactive execution on remote lfd

## Phase 3: Push notifications

APNS integration for "wave needs attention" alerts.

- lfd → loopflow.studio → APNS → phone
- Triggers: wave waiting, completed, errored, PR ready
- Tap opens that wave in app

## Phase 4: Chat experience

LLM-powered conversation about code. No tools, no execution — just discussion.

- Context assembled from wave state (area, diff, PR)
- Suggestions bridge to Phase 1/2 actions
- Claude/OpenAI/Gemini API (user's choice)

## Phase 5: Agent harness

Chat gains tools. Full agent on phone.

- File read/write, bash, git (via lfd API)
- Structured permission prompts
- Unified across LLM providers

## Architecture

```
LoopflowCore (shared)
  ├── Models, protocols, noun views
  │
  ├── Concerto (macOS) — local + remote lfd
  └── ConcertoMobile (iOS) — remote lfd only
```

LoopflowCore is the shared layer. iOS-specific workflow views (triage, create) live in ConcertoMobile. Noun views (WaveCard, StatusBadge, ActionButton) are shared.

## Items

| Doc | Phase | What |
|-----|-------|------|
| [ios-conduct-ui](ios-conduct-ui.md) | 1 | Wave dashboard on iOS |
| [ios-improvise-ui](ios-improvise-ui.md) | 2 | Create and trigger waves |
| [push-notifications](push-notifications.md) | 3 | APNS alerts |
| [chat-experience](chat-experience.md) | 4 | LLM conversation |
| [agent-harness](agent-harness.md) | 5 | Agent with tools on mobile |
| [remote-terminal-view](remote-terminal-view.md) | Deferred | Terminal streaming |

## Reference

Personas: `.lf/directions/{conductor,improviser,listener}.md`
Remote infrastructure: `roadmap/remote/`
