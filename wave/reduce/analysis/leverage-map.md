---
head: d370450a81ccdd4b8c00f3052f8271e3b515a575
status: bootstrap
---

# Leverage Map

## Ranking

1. Prompt and context creation.
2. Loopflow using loopflow.
3. Session and worker lifecycle reliability.
4. Agent computer-use harnesses.
5. DTO/API contract discipline.
6. Documentation and UI consistency loops.

## 1. Prompt and context creation

**Why it compounds:** Every step, flow, worker, review, and wave iteration is
bounded by the context it receives. Better context improves all work without
changing each worker.

**Current clues:**

- `PROMPT_STYLE.md` is detailed and opinionated.
- `.lf/steps/`, generated skills, and external-agent invocations all carry
  related prompt material.
- `token-compress` exists because long-running handoffs already hit context
  pressure.

**Reduce angle:** Build one map of context sources, inclusion rules, and
compression obligations. Then remove duplicated or stale prompt variants.

## 2. Loopflow using loopflow

**Why it compounds:** The product's own governance is the harshest integration
test. If waves can run the repo, the architecture is real.

**Current clues:**

- `wave/` already tracks goals, release, workflows, desktop, website, root, and
  reduce.
- The reduce wave itself needs durable analysis, proposals, and a queue.
- Release notes and PM sync already use repo-authored process artifacts.

**Reduce angle:** Make the internal wave system a first-class fixture. Find
manual rituals that should be authored as flows, triggers, or wave state.

## 3. Session and worker lifecycle reliability

**Why it compounds:** Every autonomous loop depends on launch, observe, cancel,
resume, and account-for-work being boring.

**Current clues:**

- `lfd` owns scheduler, executor, queue, triggers, events, store, and session
  types.
- Concerto has session, run, terminal workspace, multiplexer, and attention
  stores.
- Tests cover concurrent clients and live API smoke, but the lifecycle spans
  Rust, Python, Swift, tmux, and external agent processes.

**Reduce angle:** Identify one canonical lifecycle state model and collapse
near-duplicates between run/session/wave/attention views where they are naming
the same real state.

## 4. Agent computer-use harnesses

**Why it compounds:** UI and browser work becomes much cheaper when agents can
inspect and act through deterministic harnesses.

**Current clues:**

- Website tests use Playwright through `website/dev.py`.
- Concerto has UI tests, screenshot generation, and verification scripts.
- gstack browser skills exist in the agent environment, but the repo-level
  harness story is not yet one concept.

**Reduce angle:** Treat browser/computer-use as an execution substrate, not a
bag of scripts. The simplification is one contract for "observe, act, capture
evidence."

## 5. DTO/API contract discipline

**Why it compounds:** Wire drift creates invisible splits between daemon,
clients, and UI.

**Current clues:**

- DTO fixture tests exist under `tests/fixtures/dto/`.
- Style forbids hidden defaults on DTOs across Rust, Python, and Swift.
- Models exist in `rust/loopflow/src/lfd/http/dto.rs`,
  `python/loopflow/models.py`, and `swift/LoopflowCore/Models/`.

**Reduce angle:** Turn parity from a rule into a map: every DTO, owner, fixture,
and consuming UI/API surface in one table.

## 6. Documentation and UI consistency loops

**Why it compounds:** A feature only exists as a product when backend, UI, docs,
tests, and release notes tell the same truth.

**Current clues:**

- README, docs, wave docs, release notes, and Swift UI copy all explain the
  product from different angles.
- Release notes already interpret decisions against shipped behavior.
- UI and docs can drift after backend changes.

**Reduce angle:** Design a distributed consistency system: feature chains,
source-of-truth markers, and checks that catch "backend exists but UI/docs did
not move."

## First proposal candidates

- **Context source map:** one authoritative map of what every agent session sees
  and why.
- **Session lifecycle unification:** one state vocabulary across lfd, lfq, and
  Concerto.
- **Feature continuity checker:** a reduce-owned analysis that tracks whether
  backend, UI, docs, tests, and release notes moved together.
