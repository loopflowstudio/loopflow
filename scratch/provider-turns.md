# Four nouns: LfSession, AgentSession, LfProcess, AgentProcess

Jack asked on 2026-10-09 to rename Process to LfProcess and engine to
AgentProcess, and floated LfSession and AgentSession to match. This is the
design for that rename. PR #1512 lands the invariant it depends on (an engine
cannot outlive the Processes that drive it) and is not part of this work.

## The model

|            | Loopflow's own | The agent provider's |
|------------|----------------|----------------------|
| Conversation | **LfSession**: the durable record a person names, binds to a Task and resumes | **AgentSession**: the provider's conversation, identified by its thread id |
| Execution    | **LfProcess**: one `lf` invocation | **AgentProcess**: the provider's OS process (`codex app-server`, `claude`, `opencode serve`) |

- An LfSession holds one AgentSession at a time. Real data: 2,182 Sessions, 4
  with more than one thread, no thread shared between Sessions.
- An AgentSession is run by a sequence of AgentProcesses; each resume starts one.
- An LfProcess launches an AgentProcess and is its parent. Other LfProcesses may
  attach (live takeover). "Driver" stops being a noun: it is the LfProcess
  attached to an LfSession's AgentProcess.
- A turn belongs to an AgentSession. Turn events already key on (thread, turn).
- "Process" and "Session" remain the user-facing words and the shared shapes.

## Today's names

| New | Today |
|-----|-------|
| LfSession | `AgentSession`, table `agent_sessions`, `session_events.session_id` |
| AgentSession | `provider_thread` (Session column, event column), `provider_session_id` (results, evidence): two names for one id |
| LfProcess | `Process`, `ProcessLfid`, table `processes`, `process_lfid` columns |
| AgentProcess | "engine": `provider_pid`, `provider_started_at`, `provider_endpoint`, `provider_generation`, `provider_process_lfid` on the Session; `provider:<n>:*` launch receipts; `harness/engine_orphans.rs` |
| (gone) | "driver": `driver_process_lfid`, `driver_generation`, `SessionDriver` |

## What the model change deletes

- The five engine columns on the Session and the two driver columns, replaced by
  an AgentProcess record with a parent LfProcess and the LfSession it serves.
- The `provider:<n>:reserved|spawn_requested|spawn_failed|exited` receipts,
  which have no production reader after #1512; they become that record's
  lifecycle.
- Separate driver and provider generations: the AgentProcess id is the fence.
- The reaper's special scan: the rule is "an AgentProcess with no live attached
  LfProcess is ended", read from the same records the gate reads.
- The open question from #1512 of whether the checkout gate should look at an
  engine pid: an AgentProcess is in the set the gate walks.
- The second meaning of "engine". `rust/loopflow/src/engine/` stays Loopflow's
  own machinery.

## Hazards

- **`AgentSession` changes meaning.** Today it names Loopflow's record; after, the
  provider's. Rename today's type to LfSession and introduce the new meaning in
  the same commit, so no revision has the name meaning both. Sweep docs and
  skills for the old sense in that commit.
- **Recent decisions reversed.** docs/architecture-reference.md says "There is no
  replacement ... AgentProcess"; AGENTS.md defines Process as "one actual lf
  process" and AgentSession as the durable conversation. LOO-400 landed the
  Process/LFID vocabulary on 2026-10-07 with Jack's approval.
- **Every open branch conflicts.** The rename touches ~60 Rust files for
  `Process` alone, `process_lfid` in ~60 more, Swift mirrors and DTO fixtures.
- **Shipped records stay.** Applied migrations, release notes and retained
  capture formats keep their old words; `#[serde(rename)]` already preserves
  `origin_exec_id` for the same reason.

## Open questions

- Does AgentSession need a table, or is it a typed id (`AgentSessionId`) on the
  LfSession and on turn events? A typed id is enough for every current use:
  resume, turn keys, and usage by provider account.
- One `processes` table with a `kind`, or a second table for AgentProcess? One
  table means `lf top`, `lf monitor` and the gate see agent processes with no
  new reader; LfProcess-only columns are empty on agent rows.
- Do wire field names change (`process_lfid`, `session_id` in `lf --json`)? The
  DTO rule means Rust, Swift and fixtures move together or not at all.
- Does one AgentProcess ever serve several LfSessions? Codex is one-to-one in
  real data; OpenCode is unchecked.
- CLI and product text: `lf monitor` and gate messages say "Process"; AGENTS.md
  says product text uses Session and Run. Keep the short words there.

## Order of work

1. Pure rename, no behaviour change, one commit each so the diff is mechanical:
   `Process` to `LfProcess`; today's `AgentSession` to `LfSession` with
   `provider_thread`/`provider_session_id` to `AgentSessionId`.
2. AgentProcess record: migration, writers at the three harness spawn sites,
   backfill from the Session columns.
3. Move readers (gate, reaper, resume admission, `lf top`) onto it; delete the
   Session columns, driver type, launch receipts and `engine_orphans`.
4. Docs, AGENTS.md, Swift mirrors and DTO fixtures in the commit that changes
   each name.

Demo: `lf top` lists a running agent's `codex app-server` as an AgentProcess
under the LfProcess that launched it; killing that LfProcess removes both within
two seconds, and `lf task status` never reports a blocker that is not a row in
that list.
