# Open coordination questions

## Architecture contract

- PR #1073 currently exposes the `WorkStatus` enum but no
  `status(work) -> WorkStatus` derivation/query. Agent-api cannot replace
  `waves.rs` status or return shared start results until that controller API
  exists; it must not derive the same state independently.
- PR #1073 stores `HomeId` on `Run` and `Launch`, but exposes no authoritative
  pre-Run `WorkRef -> HomeId` placement. `lf start` cannot select local versus
  SSH execution for a never-run Wave until architecture owns that relation.

- What exact typed event replaces the shipped per-Wave process
  `lf stop <wave>` after one Home resident hosts many Work endpoints?
  Agent-api proposes `UserStart` through existing `WaitOn::Event`, resolved by
  `lf start`, with useful work reserved by `RunTrigger::User`. The invariant is
  more important than the name: stopping
  one Wave must not kill the Home resident or unrelated Work, and starting must
  not generically clear other Waits. Stop may supersede the current scheduling
  Wait while retaining its history; Start must re-evaluate current reality
  rather than blindly restore or discard that Wait.
- What final records own `HomeId`, mutable routes, Home-to-User control, and
  repo-path mapping? Agent-api must query those records; it must not preserve
  `WaveHome` address text as identity. In particular, `lf start` needs an
  authoritative `WorkRef -> HomeId` placement before a first Run exists;
  historical `Run.home_id` cannot answer that question.
- Does the shared Work controller own cross-Home child dispatch? Agent-api
  should start selected Wave roots on their configured Homes and let the
  controller place Project/Task Work. If callers must activate every Home in a
  Work subtree, architecture should expose one operation for that plan so
  `lf start` does not become a second recursive scheduler.
- What is the final repository/portfolio wrapper around `status(work) ->
  WorkStatus`? Agent-api should add scope only, not parallel Wave/Project/Task
  status nodes.

## SSH authority

- What public flag should select the product worktree's existing
  `AccountAuthoritySource::RemoteNative` for a nested detached command?
  `--remote-native` is the design spelling. It must suppress provider, GitHub,
  Linear, and named-secret forwarding while leaving SSH connection auth intact.
- Can the final Home resolver support `lf ssh <HomeId>` directly? That form
  should resolve destination and port, carry the id as a non-authoritative
  assertion, and compare it with the remote's durable local Home before the
  nested command mutates state. Raw SSH destinations can remain for ad-hoc use.

## Product behavior

- Which Waves does bare `lf start` select once Home identity no longer embeds a
  username? The intended rule is Waves on Homes controllable by the current
  User; `--all` is the explicit broader attempt.
- Direct Mac attachment to a remote loopback listener is not covered by remote
  lifecycle. Agent User operations can route through `lf ssh`; Mac remote chat
  needs a separate transport decision.
