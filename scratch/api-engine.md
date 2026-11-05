# The engine and its adapters: one operation set behind the CLI and the server

LOO-593 · design, November 5, 2026 · Jack Heart's direction, written up by the Task operator

## Direction

Jack Heart, reviewing the revision-2 API on November 5:

> API should obviously not be a standard-out scraper. The standard out should
> almost be inverted. The standard out is taking the API that we would send to a
> product, to a client, and then just printing it out to standard out. It's more
> like a certain LF CLI command that kind of proxies into the API engine and then
> exports it in a different way.

> We basically want to take today's CLI, make it available through an API instead
> of through a CLI, and then convert the CLI into an adapter (a peer adapter to the
> WebSockets or Unix server API adapter) on top of an inner core engine for
> execution.

> Things should certainly be namespaced. We should look at both the config and
> today's CLI as places where we've tried to namespace things. We should align all
> of those to make sure that the config, the CLI, and the API all have the same
> name spacing.

Jack also approved the dotted resource names (`task.create`, `task.update`,
`wave.sync`, …) and the `desktop.*` namespace for app-owned arrangement state.

## What exists today

```text
Desktop ── server (revision 2) ── Write::argv() ── Clap ── handler ── stdout ── parse
CLI ─────────────────────────────────────────────── Clap ── handler ── stdout
```

The CLI is the implementation. The server re-enters the Clap parser with a
synthesized argv and scrapes the handler's stdout. Consequences, all found in the
November 5 review of `lf/commands/{read,write}.rs`:

- 58 operations, 33 reads and 25 writes, named after CLI spellings
  (`DesktopTask`, `TaskFromSession`, `MonitorPs`).
- No typed write results; `command` answers `{trace_id, code, value, output, stderr}`.
- The core loop is absent from the API: nothing runs a Task, approves a ship,
  steers a Session or answers an agent's question. Writes cover bookkeeping
  (complete, reopen, notes, move, claim, abandon) and Desktop layout.
- Two reads (`WaveDetail`, `Roadmap`) sync with the tracker as a side effect
  while `PlanningRefresh` exists as the explicit verb.
- Two dry-run patterns (`BindPreview` read; `NewChapter.preview` field).
- Two configuration paths (`Operation::Configuration` and `Read::Settings`).
- Thirteen Desktop-arrangement operations mixed into the Work surface.

The revision-2 follow-up pass on PR #1813 (running now) fixes the wire-level
leftovers: dotted names, typed write results for the existing 25 writes, typed
checkpoints, no `generation`, client ids, domain error codes, the `desktop.*`
namespace. It does not add the core loop and does not touch the CLI tree or
config keys. That is this design.

## Target shape

```text
                       ┌──────────── engine: operations ────────────┐
                       │ task.create  task.run  task.ship  task.steer │
                       │ session.*  wave.*  graph.*  machine.*  …     │
                       │ typed input → typed result | typed error     │
                       └───────┬──────────────────────┬──────────────┘
          CLI adapter ─────────┘                      └───────── server adapter
   lf task run LOO-1 --json                         request {id, operation: {kind: "task.run", …}}
   prints the result in text or JSON                 Unix socket locally, WebSocket remotely
```

Rules:

1. **One operation set.** Every operation is a Rust function with a typed input,
   a typed result and a typed error. The engine knows nothing about argv, stdout,
   sockets or JSON framing.
2. **Adapters are thin and peers.** The CLI adapter parses argv into an input,
   calls the operation, and prints the result in the chosen format. The server
   adapter decodes a message into the same input and encodes the result as
   `response`. Neither adapter owns logic the other cannot reach.
3. **Names are the same in three places.** The API kind, the CLI path and the
   config key share one namespace: `task.create` is `lf task create` and any
   setting for it lives under `task.`. Where they differ today, the CLI and
   config move to the API's name, because the API is what clients and docs see.
4. **Reads are pure.** Anything that talks to the tracker is a verb
   (`wave.sync`), never a side effect of a read.
5. **One dry-run pattern.** A `preview` field on the write, never a separate read.
6. **Desktop arrangement is its own namespace.** `desktop.*` is app-owned state
   lf stores for the Mac app. Documented as such; not offered to a phone or a bot.

## The core loop operations

These are what a phone, a Discord bot or Rex driving Loopflow needs and the API
lacks. Each already exists as a CLI command; the work is to lift it into the
engine so both adapters reach it.

| Operation | CLI today | Result |
|---|---|---|
| `task.run` | `lf task run <issue> [--from <step>] [-a <agent>]` | the Graph process started |
| `task.ship` / `task.hold` | `lf ship` / `lf ship --stop` | the Task's run control |
| `task.steer` | `lf task comment <issue> --steer` | the saved steer |
| `task.comment` | `lf task comment <issue>` | the comment |
| `task.wait` | `lf task wait <issue> --until` | the state reached (subscribable) |
| `task.interrupt` | `lf task interrupt <issue>` | acknowledgement |
| `session.steer` | `lf session steer <id> <message>` | the saved steer |

The phone's feature is the Session's real terminal, attached through
`terminal.attach` (LOO-718). Jack Heart (November 5): "I want to have an actual
terminal on the actual Claude Code session." There is no asks reading and no
write that types into a Session on a person's behalf: what a Session is waiting
on is already the `availability` field of `session.list`, and answering is
typing in its terminal. A `session.asks`/`session.answer` pair built on November
5 was reverted on that direction.

## Namespace alignment

Observed on November 5 (`lf help`, `.lf/config.yaml`, `config/catalog.rs`):

- CLI nouns: `task wave session machine server desktop config graph` plus
  loose verbs at the root (`ship`, `portfolio`, `usage`, `history`, `doctor`,
  `open`, `monitor ps`, `ops people`, `ops tokens`).
- Config keys: `exclude skills planning linear release` at the top of
  `.lf/config.yaml`; the typed catalogue uses one prefix, `context.`.
- API kinds after the follow-up pass: `task. session. wave. chapter. plan.
  repository. graph. machine. process. usage. history. work. config. view. desktop.`

Mismatches to resolve, each by moving the CLI or config to the API name:

| API | CLI today | Change |
|---|---|---|
| `task.update` | `lf task edit --notes` | `lf task update` |
| `wave.create` | `lf wave add` | `lf wave create` |
| `wave.sync` | `lf wave status --sync` | `lf wave sync` |
| `wave.get` | `lf desktop wave` | `lf wave show` already exists; Desktop read becomes `wave.get` |
| `task.get`, `task.list` | `lf desktop task`, `lf desktop done` | `lf task show`, `lf task list --closed` |
| `session.list` | `lf desktop sessions` | `lf session list` already exists; one implementation |
| `view.list`, `view.get` | `lf desktop plans`, `lf desktop view --plan` | `lf view list`, `lf view show` |
| `repository.people`, `repository.tokens` | `lf ops people`, `lf ops tokens` | `lf repository people`, `lf repository tokens` (`ops` dissolves) |
| `process.list` | `lf monitor ps` | `lf process list` |
| `machine.doctor` | `lf doctor` | keep the root alias; canonical path under `machine` |
| `task.hold` | `lf ship --stop` | `lf task hold`; `lf ship` stays the short form of `task.ship` |
| `config.*` | `lf config …`, `planning:`/`linear:` keys | planning settings move under `planning.` in the typed catalogue; `linear:` becomes `planning.linear.` |

The follow-up pass records the complete list in `scratch/questions.md` as it
renames each operation; this table is the shape, that list is the inventory.

## What this deletes

- `Write::argv()` and the argv round trip into Clap from the server.
- Stdout capture, the output limit and the "parse if the spelling had --json" rule.
- `ops` as a CLI noun; `desktop` as a home for Work reads.
- `Read::Settings` or `Operation::Configuration`, whichever loses.

## Risks

- **Scope.** 58 operations plus the core loop touch most of `lf/`. Land it as
  one Task per namespace (`task.*` first, since it carries the core loop), each
  deleting its slice of the scraper as it goes.
- **CLI renames are user-facing.** Keep the old spelling as a Clap alias for one
  release where a documented command moves (`lf task edit`, `lf wave add`,
  `lf ship --stop`), and delete the aliases with a filed Task. `documented_commands`
  checks the examples resolve.
- **Config keys are shared files.** `.lf/config.yaml` is read by the released lf
  and a branch build; per AGENTS.md the branch keeps the released shape readable
  until the new shape ships, with the removal Task filed in the same change.

## Demo

From a phone-shaped client (the Swift test peer is enough): connect, `session.asks`
shows a Claude Session waiting on a question, `session.answer` answers it, the
terminal shows the agent continuing. From the CLI: `lf task run LOO-1 --json`
prints exactly the `task.run` result the server would send. One operation, two
adapters, same bytes.

## Sequence on this branch

Jack Heart (November 5): "do this all here." Everything below lands on PR #1813,
one commit series per step, each verified with the server suites and the CLI's
own tests before the next begins.

1. **task.\* and the core loop.** Lift `task.run`, `task.ship`, `task.hold`,
   `task.steer`, `task.comment`, `task.wait`, `task.interrupt` into the engine;
   CLI handlers become adapters; delete the scraper for the task namespace.
2. **Reverted.** An asks reading and answer write were built and removed; the
   terminal is the whole phone interaction.
3. **Namespace alignment.** The table above: CLI renames with one-release aliases,
   config keys under the typed catalogue, `ops` dissolved, `desktop` left with
   arrangement only.
4. **Remaining namespaces.** `wave.* session.* graph.* machine.* repository.*
   usage.* history.*` through the engine; fold the two config paths into one.
