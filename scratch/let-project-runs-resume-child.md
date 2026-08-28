# Restore Project child-resume authority

## Finish line

A Project controller reaches `project/pursue`, invokes the existing
`lf task resume` path for its parked Task, and receives an idempotent success.
The same mutation is refused for another Project, for superseded controller
authority, and after new Project direction makes the active phase stale.

The distinguishing proof is a deterministic fixture that crosses both a
Project phase transition and controller-process replacement, then exercises
the same authorization entry point used by Task resume. Merely accepting a
generic `LF_RUN_ID`, trusting Run-record subject attribution, or adding a
special resume bypass does not count.

## Observations

- The reported 2026-08-19 failure came from the retired SQL
  Run/Invocation/Turn/Basis lifecycle. Main removed that lifecycle in #1237;
  generic Home-local Run records are now evidence and causality only.
- Current Project and Task controllers both publish their Run manifest before
  starting the harness. Capture publication failure already ends the Project
  before any provider phase starts.
- A Project provider inherits `LF_RUN_ID`, but `lf task resume` currently uses
  it only as `Author::Run` provenance. No scoped Work-mutation authority is
  checked before the Task controller launch.
- Task controller launch is already retry-safe on one machine: the stable tmux
  session name makes a repeated resume a no-op when the controller is live.
- Project Steers remain an ordered durable stream with a contiguous sequence.
  That sequence is the smallest current equivalent of the former Turn Basis:
  a phase prepared at sequence N must stop controlling children after steer
  N+1 arrives.
- Run-record subject attribution cannot safely fill the gap. The current
  architecture explicitly makes attribution non-authoritative, and a generic
  Run id alone does not prove Project ownership.

## Hypothesis

Issue one opaque Project child-control capability from planning SQLite before
the first provider turn. Store only its hash, binding it to the exact Project,
controller Run, flow step/iteration, and Project Steer sequence. Rebind the
same capability at every Project phase boundary; replace it when a recovered
controller publishes a new Run. `lf task resume` accepts local User authority
unchanged, but an in-Run caller must present this capability and match the
Task's immediate parent Project and current Project Steer sequence.

This keeps `RunId` as provenance rather than authority: the random capability
and its planning-store binding grant the mutation. It also avoids resurrecting
the deleted SQL execution lifecycle.

## Material assumption

LOO-227's requested “Turn Basis” is interpreted on current main as the exact
Project flow step plus the ordered Project Steer frontier. Reintroducing the
retired AgentInvocation/Turn schema would conflict with #1237 and is larger
than restoring this surface.

## Near misses

- Authorize every in-Run caller that names the Task's Project.
- Parse Project identity from Run-record subjects.
- Treat a missing capability as User authority.
- Mint a capability only in `project/pursue`; clarify or mutate can transition
  into supervision without restarting the provider process.
- Special-case `lf task resume` after it reaches the launcher.
