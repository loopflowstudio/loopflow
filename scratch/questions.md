# LOO-582 decisions and assumptions

Jack Heart's November 4, 2026 direction selects one Session cwd immediately on
binding. All pending-directory and preview proposals are superseded. No product
choice remains open. The accepted contract and deletion list are in `loo-582.md`.

Implementation assumption (November 4): use existing connection paths for
provider cwd updates. OpenCode's native move endpoint is owned by its connection;
binding has no exposed live provider-control endpoint. Native clients may keep
their launch directory, as Jack explicitly accepted. No new transport control,
restart/draft-transfer machinery, or implicit interruption is introduced.

Retained decisions from November 3: closed Sessions and headless Runs cannot be
Task primaries; done unplaced Tasks bind in place and stay done. Explicit sources
are required for create adoption. Plain create and create-and-run file separate
work. Permanent conflicting assignments remain intact.

The active Graph owns publication to PR #1807 and the interactive demo, then the
combined gate. No merge or Auto-Ship approval. Desktop create-from-chat draft
retention and the changed candidate's configured behavior remain demo proof.
Older observations, including native detached draft retention and the repaired
cmux hostname issue, remain in `loo-582-demo.md`; Warp access was refused. The
complete pre-reduction notes are at `345a8d919c:scratch/questions.md`.

Deletion finding (November 4): the old missing-destination fixture proved the
pending-directory admission gate. Ordinary reconnection can create local launch
scaffolding at its selected cwd; the removed gate no longer intercepts that path.
Delete that exclusive fixture rather than restore a binding-specific gate.
Provider-move rejection still proves failed connection retries retain cwd and
native identity. No files are relocated or inputs replayed by binding.

Work-state gap (November 4): the final `lf task status LOO-582 --json` read fails
with "no Loopflow registry on this machine"; current Graph state is unknown.
Local implementation and checks completed independently. No registry repair or
new execution was attempted. Publication remains the authored Graph's step.
