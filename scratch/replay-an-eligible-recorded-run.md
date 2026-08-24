# Replay an eligible recorded run unattended

## Problem

`lf replay check` can prove that one recorded AgentInvocation has a complete,
immutable, locally authoritative replay contract. It cannot execute that
contract. The Trace Project therefore has evidence that replay inputs exist but
no evidence that those inputs can produce a second, independently inspectable
run.

LOO-272 closes that gap for the only producer that currently emits replay-safe
contracts: a fresh unattended native-Codex invocation. A replay must use the
recorded commit, prompt bytes, context manifest, provider runtime, managed
account, config identities, process settings, argv, and selected environment.
It must not assemble a new prompt, select a current model or account, run in the
source checkout, or resume the source provider session. Refusal is preferable
to a plausible execution assembled from current state.

## The demo

Run `lf replay run <eligible-invocation-id>`. Loopflow materializes the recorded
commit in a retained isolated sibling checkout, runs native Codex unattended,
and prints the new trace and invocation ids; `lf trace <new-trace-id> --json`
shows a complete capture whose `replay_source_invocation_id` points to the
source while `lf replay check <source-id> --json` remains eligible with the same
artifact digests.

## Approach

### Turn strict preflight into an executable plan

Refactor the existing checker around one internal `ReplayInspection`. It owns
the existing `ReplayCheckDto` plus an optional `EligibleReplay` populated only
when every refusal check passes. `lf replay check` keeps its current output and
exit behavior. `lf replay run` calls the same inspection and cannot obtain
launch inputs from a refused result.

`EligibleReplay` holds decoded, already hash-checked source evidence rather than
paths to rediscover later:

- the source `AgentInvocationRow`, replay-contract index,
  `ReplayContractV1`, and `ExecutionContractV1`;
- every finalized Turn's exact system/task prompt bytes, ordered context assets,
  input operation, timing, coverage, and tokenizer;
- the normalized conversation status that proves Turn order, supported input
  boundaries, and terminal completeness;
- the exact effective-Codex configuration artifact, runtime/config identities,
  and the complete set of source artifact references to recheck before launch.

Promote the already-persisted `EffectiveCodexConfigV1` shape to a versioned,
`deny_unknown_fields` replay reader. LOO-271 stores this content-addressed JSON
among `provider.config_files`; require exactly one effective-config artifact,
then prove that its argv and environment selectors equal the execution
contract, its method is `thread/start`, and its thread parameters name the
recorded model, cwd, `never` approval policy, and `workspace-write` sandbox.
Also require `provider == "codex"`, `sanitized_argv[0] == provider.binary.path`,
an `app-server` argv with no resume token, and a `CODEX_HOME` selector whose
canonical path equals the native Home of the recorded exact account. Use the
existing refusal vocabulary:
`unsupported_surface` for unsupported providers/surfaces,
`contract_identity_mismatch` for duplicated fields that disagree,
`contract_invalid` for malformed launch shapes, and the existing artifact,
authority, repository, and runtime codes for those boundaries.

Validate that each normalized `user_input` event agrees byte-for-byte with its
Turn task-prompt artifact. The conversation remains evidence of ordering and
timing; source assistant/tool output is never fed into the replay.

### Materialize a standalone recorded checkout

After the first strict inspection succeeds, create a unique sibling directory
named `<repo>.replay-<source-short>-<attempt-short>`. Use a local
`git clone --no-hardlinks --no-checkout` from the recorded canonical repository,
check out the recorded commit detached, remove the clone's `origin`, and verify
the exact `HEAD` plus a clean worktree. This copies Git metadata rather than
sharing the source repository's `.git/worktrees` directory, so provider writes,
commits, and ref changes stay inside the replay placement. The sibling location
preserves the recorded ancestor config stack; project config inside the repo
comes from the recorded commit.

Hold the unlaunched placement with bounded temporary ownership. A
pre-launch refusal removes it; once trace capture begins, retain it on success
or failure and print its path so the result remains inspectable.

The recorded cwd cannot be used literally without mutating the source. Load
the captured `thread/start` params and replace only the proven recorded cwd with
the isolated placement. Every other parameter remains byte-for-byte from the
effective-config artifact. The new invocation's own execution contract records
the actual isolated root/cwd and the same commit, provider, model, account,
runtime, process policy, argv, and environment; it is therefore honest and can
itself finalize a replay contract.

Resolve the Codex config stack once more at the isolated cwd. Its file-identity
set must equal the recorded set after the single mechanical prefix mapping from
the recorded cwd to the isolated cwd; system/native-account files keep their
absolute identities, while repository `.codex/config.toml` files come from the
recorded checkout. An added ancestor config, missing profile, or changed byte is
a refusal, not a new effective default.

### Recheck mutable identities at the launch edge

Materialization creates a real time-of-check/time-of-use interval. Immediately
before provider spawn, re-run the source artifact, runtime, config, exact-account,
Home, repository-object, and effective-config checks. Build the prepared launch
only from the retained `EligibleReplay`; never call normal prompt assembly,
model parsing with defaults, config discovery, or non-exact account routing.
Any drift returns the same typed refusal evidence and the provider is not
started.

Prepared Codex commands currently inherit non-allowlisted ambient variables.
For replay-safe prepared launches, clear the child environment first, then add
only the captured selector map, the exact native account route, and Loopflow's
derived control variables. This makes `environment_selectors` the real process
contract rather than a partial description. Do not retry, fail over accounts,
resume a vendor session, enable hooks/MCP/apps/network, or add writable roots.

### Drive every recorded Turn through the native harness

Add a prepared native-Codex replay driver beside the existing prepared-launch
path. Start the exact recorded app-server executable and argv through
`CodexHarness::start_prepared`, send the initial system/task bytes, and wait for
one terminal `turn/completed`. For each later strict-preflight-supported
`message` at a `turn_boundary`, begin the corresponding new trace Turn and send
its exact task-prompt bytes only after the preceding Turn terminates. Record raw
provider frames, normalized conversation events, usage, provider session id,
and terminal outcome through the existing `CaptureHandle`; do not copy source
outputs into the new conversation.

The command finishes the new capture on every post-capture path. Provider
failure is a complete failed replay result when terminal evidence exists;
transport/capture failure remains visibly partial or failed under the existing
trace lifecycle rules.

### Make replay lineage a first-class trace fact

Add nullable `replay_source_invocation_id` to `agent_invocations` in a new draft
migration, with a self-reference using `ON DELETE RESTRICT` and an index for
reverse lookup. Thread it through `AgentInvocationRow` and `CaptureStart`.
Capture start proves that the source exists in the same selected Home and is
not the new invocation before inserting the child row. No source row, contract
row, or artifact is updated.

`lf trace` already serializes `AgentInvocationRow`, so JSON gains the required
source link directly. Add the source id to the text invocation block as
`replay of`. `lf replay run` prints the source id, new trace id, new invocation
id, retained placement, and terminal outcome.

Update the examples in `README.md` and `docs/lf.md`, and change the replay row in
`docs/architecture.md` from a read-only checker to the checker/executor boundary.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Does the retained LOO-271 proof provide the demanded source record? | On 2026-08-24, no SQLite database under `/Users/jack/.lf` or `/Users/jack/.lf-dev` contained `replay_contracts`. The retained LOO-271 Home contains `invocation_2f2ac303607c4673a19b83f462497c5b`, but it is a complete/failed invocation with no execution/replay artifacts; the built CLI fails with `no such table: replay_contracts`. The focused synthetic eligibility fixture still passes. | The live demo must first create a fresh real eligible native-Codex source through the shipped LOO-271 producer, then replay that exact id. A fixture cannot stand in for the real execution proof. |
| Can `codex exec` be used as a shortcut? | [Official OpenAI documentation](https://learn.chatgpt.com/docs/developer-commands?surface=cli#codex-exec) describes `codex exec` as stable unattended execution, but it inherits config defaults and is a different protocol from the recorded `app-server` argv plus `thread/start` request. | Reuse the native prepared app-server harness and captured effective config. A generic `codex exec` relaunch would be reconstruction from ambient behavior. |
| Is the effective app-server request recoverable from V1? | Yes. LOO-271 persisted argv, thread method, thread params, and environment selectors in a content-addressed effective-config JSON and included its exact file identity in `provider.config_files`. The type is currently serialize-only and not semantically checked by preflight. | Add a strict V1 reader and require exactly one effective-config identity that reconciles with the execution contract before producing an `EligibleReplay`. No source schema change or backfill is needed. |
| Does prepared launch currently use only `environment_selectors`? | No. `CodexHarness::start_inner` removes and restores the 11 allowlisted selectors but leaves every other ambient process variable inherited. | `env_clear` the replay-safe prepared child, then apply captured selectors, exact account authority, and derived Loopflow control identity. Add a behavioral assertion that an unrelated ambient variable is absent. |
| Can a linked Git worktree provide isolation? | No. It shares the source repository's common Git directory outside the provider's workspace; commits and refs are not isolated even when file writes are. | Use a standalone no-hardlink local clone, detach at the recorded commit, and remove its origin. Keep the placement after launch for inspection. |
| What changes between preflight and spawn? | Repository/config/runtime paths remain mutable, and materializing the checkout widens the interval. Current `check` validates them once and discards the loaded contracts. | Retain decoded inputs, compare the isolated cwd's exact config stack with the recorded path-mapped set, then repeat all path/account/hash checks immediately before spawn. The drift fixture mutates a config after clone and proves typed refusal before the fake provider sentinel appears. |
| Can the current normal launcher preserve identity? | Not by rebuilding `AgentConfig`: normal preparation rereads config, may select a current route, builds current thread params, and creates prompts from the current checkout. Replay-safe launches already disable retry, but only after current-state preparation. | Construct one exact `PreparedAgentInvocation` from `EligibleReplay`, require the recorded account route, and drive it without retry, failover, resume, or prompt assembly. |
| How does the result become independently inspectable? | `lf trace --json` already emits invocation and Turn rows plus assets and decisions. It lacks cross-invocation replay lineage. | Store the source id on the new invocation, retain the isolated checkout, and let ordinary trace capture finalize all new artifacts. Do not create a replay results store. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Run `codex exec` with the recorded task prompt | Small command surface and officially supported unattended mode. | It changes the recorded app-server protocol, rebuilds config from defaults, cannot apply the captured `thread/start` request exactly, and bypasses Loopflow's normalized multi-Turn capture. |
| Reset or reuse the recorded worktree | Avoids cloning and preserves the literal cwd. | It mutates source state, makes concurrent work unsafe, and cannot prove source digests/worktree state remained unchanged. |
| Create a detached Git linked worktree at the commit | Fast and reuses objects. | The common Git directory remains shared and writable outside the sandbox; a replay can change source refs or require external writable roots. Isolation would be a story, not a boundary. |
| Rebuild a fresh ordinary `AgentConfig` from `ExecutionContractV1` | Reuses `launch_agent` with little new engine code. | Current builders reread ambient config and synthesize current thread params. The content-addressed effective-config artifact already contains the exact launch; throwing it away defeats the contract. |

## Key decisions

- `lf replay run` accepts only an `EligibleReplay` returned by the same strict
  inspection as `lf replay check`. There is no second eligibility predicate.
- Native Codex app-server is the only executable provider in this slice.
  Claude, OpenCode, legacy rows, and future contract schemas remain typed
  refusals.
- The source and destination ledger/Home must be identical. Development builds
  whose read authority and write store differ refuse before placement rather
  than creating an unjoinable trace.
- The isolated cwd is the sole intentional launch substitution. It is derived
  from the recorded cwd after exact comparison and is recorded honestly on the
  replay invocation; model, account, argv, environment, policy, timing, and
  prompt bytes are not substituted.
- Every eligible Turn is replayed in order. Silently replaying only Turn one
  would turn a complete contract into a partial prompt demo.
- Replay placement is retained once execution begins. The provider's filesystem
  result is evidence, not scratch cleanup.
- Lineage lives on `agent_invocations`, beside the facts it relates. There is no
  remote service, replay database, comparison record, or judge.
- Source artifact bytes are read-only inputs. The behavioral proof hashes the
  source replay contract, execution contract, prompts, and conversation before
  and after the run.

Wild success is boring: replay is one command, a refusal names one exact broken
boundary, and the resulting trace behaves like every other trace. Wild failure
is a second launcher that mostly resembles the source while quietly selecting a
new account/config/cwd, or an "isolated" checkout whose Git metadata lets the
agent affect the source. The design spends its complexity only on preventing
those two outcomes.

## Scope

- In scope: `lf replay run <invocation>`; shared strict inspection; native-Codex
  app-server execution; exact multi-Turn prompt replay; same-Home exact account
  authority; standalone local checkout at the recorded clean commit;
  launch-edge drift refusal; complete new trace capture; typed source lineage;
  text/JSON trace inspection; focused CLI behavior and real built-path proof;
  user/architecture documentation.
- Out of scope: Claude and OpenCode; legacy backfill; remote or cross-Home
  replay; retries or account failover; source provider-session resume; cohort
  sampling/statistics; context diffing, comparison, grading, or judging;
  historical gap-days; partial-capture recovery; SQLite-lock recovery; remote
  telemetry; placement pruning/retention policy; replay performance
  optimization.

## Done when

1. A focused CLI fixture invokes `lf replay run` against an eligible V1
   native-Codex contract and a fake app-server through the production harness.
   It proves the provider ran in a clean standalone checkout at the recorded
   commit, received the exact recorded Turn input(s), inherited no unrelated
   ambient variable, and produced a complete new invocation and replay
   contract linked to the source.
2. The same fixture snapshots every source replay/execution/effective-config,
   prompt, and conversation digest plus the source repository state before
   execution and proves all remain identical afterward. `lf trace <new-trace>
   --json` exposes the link and complete result without a special reader.
3. A post-preflight fixture changes one hashed config/runtime artifact after
   isolated materialization. `lf replay run` exits nonzero with the existing
   `artifact_hash_mismatch` refusal, creates no replay invocation, and never
   starts the fake provider.
4. Through the built, non-test CLI, create a fresh real replay-safe native-Codex
   invocation because the retained LOO-271 proof artifact is absent. Confirm
   `lf replay check <source> --json` is eligible, run `lf replay run <source>`
   unattended, and inspect the returned trace with `lf trace <new-trace>
   --json`. The source remains eligible and all source artifact digests are
   unchanged.
5. `cargo test -p loopflow --test replay_tests` passes, followed by `cargo fmt
   --check` and `cargo clippy --all-targets -- -D warnings` for the Rust change.

No metric is proposed. This slice changes the next Trace decision only when the
binary execution proof exists; 10/10 cohort success belongs to the explicitly
later sampling work.
