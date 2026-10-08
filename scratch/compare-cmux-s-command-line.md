# Desktop control on shared Work — LOO-427

**Status:** implementation authorized by Jack Heart; 0 implementation slices
complete. One PR through demo review. No production code has changed.
The implementation sequence below replaces the earlier combined architecture
slice. [Findings](findings.md) hold the source evidence;
[questions](questions.md) hold only unresolved product decisions.
The [comparison](../docs/reviews/terminal-command-comparison.md) retains all command
judgments and behavioral evidence.

## Accepted outcome

Jack Heart selected one repository window across machines, built on one shared
Work model, “all fromone place,” with the ability to “Delegate” a subtree to a
Machine. Machines execute that Work. They do not supply independent trees for
Desktop to merge. LOO-430–432 are canceled; their identity, arrangement and
terminal-I/O outcomes remain in this PR.

An agent can identify work, open its repository window, arrange retained panes,
read displayed output and deliberately send input to the exact pane. Closed
repositories need no window; repeated opens reuse one. Work identity, execution
location and visual placement remain distinct. Mac view control is macOS-only;
Linux gets an actionable error before launch or Work changes. Ordinary lf remains
cross-platform. No live planning migration is authorized.

## Architecture and ownership

| Fact | Authority | Derived consumers |
|---|---|---|
| Repository/Work identity, hierarchy, authored delegation | One shared Work source; physical source/write protocol is Q1 | CLI resolution, repository window, scheduling/routing |
| Checkout Machine/path and Process history | Recorded execution facts, preserved independently of delegation | Task files/status, Session association, runtime observation |
| Window, panes, focus, retained native surfaces | Existing Desktop registry, multiplexer and surface pool | Programmatic inspection/arrangement/input |

Adapt `Placement` and existing Work routing. Do not add repository-pairing groups,
a Desktop Work database, a second layout store or a terminal emulator. Source
loss cannot manufacture an empty plan or turn a cache into write authority.
LOO-411's old SSH `--repo` removal is not a veto on Work-directed routing.

LOO-406 owns the local planning lifecycle; LOO-412 supplies remote checkout/adoption
work but does not yet supply shared IDs for all existing Tasks. LOO-426 owns Work
opening; LOO-416 saved panes; LOO-387 draft preparation; LOO-415/424 relay/resume;
LOO-422 host status; LOO-402/403 Waiting/shortcuts; LOO-397 command discovery.
These boundaries are integration constraints, not separate replacement projects.

## Implementation sequence — one PR

### 1. Separate recorded checkout location from delegation — This slice

**Ready. Depends on no shared-source choice.** Preserve current routing behavior
while removing the concrete coupling that makes delegation unsafe.

- Keep authored machine assignment in the existing placement owner. Store the
  Task checkout's actual Machine alongside its recorded path in the existing
  checkout/Task representation; do not introduce another generic placement table.
- Backfill recorded location from the pre-change placement facts for existing
  checkouts. Preserve unresolved historical evidence as unknown, never infer it
  from the executing process. Preserve Work, PR, Session and Process IDs.
- Cut over `SqliteStore::task_checkouts`, `task_snapshot`, Task-file access and
  checkout-based Session association together to recorded checkout location.
  Update their wire fixtures/Swift consumers wherever the changed contract reaches.
- **Delete — do not maintain:** checkout Machine inference from mutable
  `work_placements`. Retain current creation-time assignment behavior until slice
  2; this slice does not silently select new inheritance or move running work.
- Use one Task migration draft against the released frontier. Later slices edit
  that same draft; no intermediate schemas ship.

**Focused proof:** migrate a fixture with a Task checkout on A, change its authored
assignment to B, and verify files, status and Session association still identify
that checkout on A. A new checkout explicitly prepared on B records B. Verify
unchanged identities and unknown-history handling. Build changed Rust and run
these behavioral tests; no installed-store experiment.

### 2. Shared Work identity, delegation and routing

**Depends on 1 and Q1; Q2 affects assignment changes.** Carry exact shared IDs
through planning reads/writes, delegation and remote dispatch. Add the repository
root identity within the shared model, including repositories with no Tasks.
Reuse LOO-406's storage primitives rather than implementing its local lifecycle
again. Preserve connected-plan ownership and existing historical identities.

Resolve Work → effective delegation → execution checkout. Define explicit versus
inherited provenance, assignment changes and unavailable-source behavior. A
Machine can execute work from several repositories without rewriting its saved
default per request. Adapt LOO-412's transport; do not mint a second authoritative
Work tree on the receiving machine. Replace independent adoption/creation paths
that would violate this contract in the same cut, retaining historical execution.

**Proof:** two machine fixtures read identical Work IDs; an authorized write is
visible through the shared source; delegated operations reach the correct checkout;
source/transport failure preserves truthful stale/unavailable state. A delegation
edit never relabels an existing checkout or claims a process moved.

### 3. One repository window and explain/inspect

**Depends on 2.** Key the window to shared repository identity. Cut startup,
restoration, menu opening, Task links and in-window navigation over together.
Replace `LoopflowApp`'s unaddressed workspace `WindowGroup` and the separate
`WavesView` opening path. Retain utility windows and existing native surfaces.

Replace `WorkLinkRouter.deliver`'s unrelated-window fallback and single pending
URL with per-repository pending destinations. Preserve every cold-open request;
concurrent opens of one repo converge. Remote readings retain Machine/freshness
without becoming another Work authority.

`explain_context(selection)` returns Machine/repo/checkout/Wave/Task/Session/Process
and selection provenance, including absent/unavailable values. `inspect_desktop`
returns selected work, layout, focus, supported operations and existing Rust
legal actions. Explicit targets and checkout inference share the same resolver.

**Proof:** cold opens for two repositories arriving before registration and
registering in reverse order both reach their own windows; same-repo opens reuse
one. Plain-repo and remote-unavailable cases remain useful. JSON/Swift agree.

### 4. Arrange and interact with exact retained panes

**Depends on 3; Q3 affects Session input.** One `DesktopTarget` contains repository,
window incarnation, Machine/workspace, pane and expected content incarnation.
Validate and dispatch on the main actor; delayed requests never follow new focus
or a reused surface. Replace path-only read keys and unqualified terminal keys
with their consumers/callbacks/fixtures together.

Expose focus, shell/Files/Flow-log companions, split, move, resize, zoom, hide and
restore through `SessionsWorkspaceRegistry`, `MultiplexerStore` and
`GhosttySurfacePool`. Native surfaces/drafts survive arrangement. Ending a
shell/client is a distinct effect from hiding a view or completing Work.

Expose bounded screen, scrollback and selection reads, distinguishing empty,
hidden and unavailable. Reads claim no client. Literal insertion and key submission
are separate; no silent draft replacement. Bounded observation establishes output,
not semantic Task/turn completion. Transport/spelling are implementation choices
using existing owners; no socket or cross-version negotiation is required.

**Proof:** stale targets, focus changes, surface replacement, retained drafts,
literal text versus Enter, passive reads and limits. Linux errors precede any
app/provider/Work effect. Use owned fixture terminals and fake providers only.

### 5. Complete-path verification and demo

**Depends on 1–4.** No partial slice ships separately. Gate runs affected Rust
behavior/migration suites and
`cargo test -p loopflow --test dto_fixtures` for Rust wire fixtures, plus
`cargo test -p loopflow --test documented_commands` for command ambiguity only.
Use `scripts/test_desktop.sh -Xswiftc -gnone --no-parallel` for headless Desktop
build/tests. Select exact focused Rust test names when the tests are introduced;
empty filters are not proof. Clear inherited execution markers per TESTING.md.

Demo: one repository window shows two Tasks on different Machines. Explain their
identity/delegation, open them, add shell/Files panes, retain an unfinished draft,
change focus, target a harmless command, read output, hide/restore, and verify the
original input target and draft survive. Review native usability separately from
headless gate. Preserve comparison dispositions and evidence limits; no real
provider accounts or live user terminals.

Check: local-link/whitespace validation, `git diff --check` and `lf context
--skill kickoff --json` pass; prose-only, no build. Implementation/native proof remains.
