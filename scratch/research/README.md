# WaveChat / wave-server design research

Prior-art研究 mined for the reactive wave server (queue / interrupt / steer,
data model, durability, supervision UX, agent-design philosophy). Each file is a
standalone brief; `../wavechat-review.md` is the synthesis that draws on them.

## Control flow (queue / interrupt / steer)
- [codex.md](codex.md) — Codex SQ/EQ, item lifecycle, steer-as-queue, interrupt
- [opencode.md](opencode.md) — Runner state machine, log-as-queue, abort finalize
- [humanlayer-control-flow.md](humanlayer-control-flow.md) — approvals, interrupt+continue, durable decisions

## HumanLayer daemon (closest prior art)
- [humanlayer-daemon.md](humanlayer-daemon.md) — hld architecture: process/store/bus/API
- [humanlayer-lifecycle.md](humanlayer-lifecycle.md) — session state machine, identity, resume/fork, durability
- [humanlayer-codelayer-ux.md](humanlayer-codelayer-ux.md) — CodeLayer/WUI supervision UX

## Philosophy
- [12-factor-agents.md](12-factor-agents.md) — the 12 factors vs our design
- [dexhorthy-talks.md](dexhorthy-talks.md) — context engineering, compaction, spec-first, outer loop
- [dexhorthy-twitter.md](dexhorthy-twitter.md) — through-lines + the event-driven "game server" thesis

## The one idea everything converges on
**A wave is a reactive fold over one append-only event log.** The log is truth;
the conversation, memory, run/wave status, and per-consumer projections are all
derived. Steering enqueues events; the agent-context projection filters queued
user messages until a pass boundary; resume/fork/replay/durable-pause fall out.
