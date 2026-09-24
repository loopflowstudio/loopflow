# Assumptions and remaining proof

- Gate 2026-09-23: affected suites stopped at resource preflight (active main
  build 19.1 GiB / 12 GiB); documented recovery could not remove it. Architecture,
  Swift boundary, formatting, and whitespace checks passed, but product suites
  and Clippy remain unrun for this gate. Concurrent snapshot edits require a
  settled-tree rerun. The subsequent slice review repaired partial-directory
  discovery and proved healthy output/Watch/shared Run reads plus recovery.
  See `review-slice.md` for that evidence and the remaining full-Task boundary.

- Product / Desktop ownership is resolved by the Task directive; no PM mutation
  is needed. This remains one PR, with the pinned human and settlement gates.
- Flow facts, launch-owned native source receipts, passive readers, incremental
  Task output, the shared Watch snapshot/CLI/Swift reader, and their DTO fixtures
  are implemented. Mac Watch now inspects plans/attempts with manual refresh,
  stable selection and stale evidence. Separate history/live cursors, bounded
  snapshot/discovery, the live feed/Follow live, human Session navigation, and
  the configured demo remain. No automatic poll is installed yet.
- The primary workspace now owns Watch alongside details and retained terminals.
  Completed Tasks remain in its shared projection with a visibility toggle/search;
  this does not recover Tasks absent from the roadmap read. Watch selection is
  per Task/window/repository, and its reader unmounts while hidden.
- Flow Session IDs are distinct from their provider Run IDs. Exact checkpoint
  navigation needs an explicit shared association; do not match by skill name,
  synthesize a Session ID in Swift, or add a separate Session inventory.
- A configured Codex reader observed new prose and tools with the original owned
  client unchanged. Claude and OpenCode have local reader/source tests only;
  all-provider live coverage is not established. No replacement client was used.
- Native sources can persist at message/tool boundaries. Live output must arrive
  before Session completion; token-by-token capture is not promised.
- Discovery currently reads all immutable Run manifests and Task flow facts on
  each request, while transcript pages are bounded. Incremental discovery and
  historical/live cursor separation still need implementation and measurement.
- OpenCode continuation detects source replacement, Session creation changes,
  reduced part counts, and removal of retained boundary/unfinished parts. An
  equal-count rewrite of older completed parts needs further investigation before
  claiming arbitrary compaction coverage; do not hide it behind a quiet source.

- Summary-only journal attempts now report `limited_capture`: their normalized
  stream lacks complete tool results. The full Task still requires resolving
  those autonomous/auxiliary capture paths; explicit gaps are failure handling,
  not satisfaction of complete output coverage.

- Cursor transport now uses `--cursor FILE` or stdin (`-`), including the Swift
  caller. Completed interior OpenCode history no longer accumulates in cursor
  state. Same-timestamp/unfinished parts and per-Run state can still hit the 4 MiB
  bound; bounded discovery and independent history/live continuation remain.
- OpenCode now freezes a sweep timestamp and retains both inclusive boundaries
  until pagination ends. This handles boundary edits interleaved with newer
  writes; it does not prove arbitrary completed-history rewrites with backdated
  timestamps or equal-count compaction.

---

# Retained main-view-task evidence (LOO-291)

# Open decisions and limits

## Implemented interpretation

- New conversation follows visible repo/Wave/Project/Task scope and honors lf's configured destination. It starts a fresh conversation without replacing existing Sessions.
- All work uses repository launch context; returning to prior details restores that subject.
- New terminal is an ordinary shell grouped by checkout. Both split levels remain independent.
- Main-view owns unified navigation; unmatched Sessions remain reachable. No one-current-conversation policy is enforced.
- Shell attachment uses actual PTY evidence, never cwd/title inference. Clients predating this marker need a fresh launch before they can report local attachment.

## Open product decisions

- Whether to add an explicit New task/worktree action alongside New conversation. The user's latest scoped-conversation proposal reopened automatic creation.
- Whether and how an exploratory checkout becomes a Task without replacing its files, conversation, or terminals. Actual Tasks require an existing Project.
- Conversation lifecycle/cardinality: general subject conversations must coexist with manual agents and independent exploratory worktrees.
- App-relaunch layout/process persistence, beyond retention within a window's lifetime.
- Integrated ownership is Product → Desktop → LOO-291; do not create a second Task.

## Remaining live proof

- Test an actual configured app launch and a terminal-configured provider launch through the new action; current native checks use harmless commands and fixture records.
- Installation and engbot cleanup are recorded in the integration handoff. Keep list historical: it owns abandoned LOO-276 and must not be hard-deleted.
- LOO-291's separate external-workflow trial and broader acceptance criteria are not completed by integrating its committed navigation here.


## Integration decisions — 2026-09-23

Latest human steering supersedes the older Task-first/cardinality assumptions:
use fresh scope-aware New conversation and separate New terminal, preserving all
existing Sessions. lf-new is now integrated code, including nested worktree
layouts and live PTY attachment, not merely a design snapshot. Bounded lifecycle
and explicit Task creation/adoption remain open; do not enforce one-current
cardinality by hiding or stopping clients. See
`lf-new-implementation/integration.md` for reconciled proof and remaining scope.

The supplied installed screenshot establishes a real appearance defect. The
integrated candidate has readable light/dark captures; the installed app has not
been replaced by this pass. Its New conversation confirmation remains pending.
Do not equate a static capture or fixture with that configured interaction.
