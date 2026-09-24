# Release notes ownership

Starting head: `41e6f4ad9af58f35ef3df7b8a6e2220eb78617ba`.

## Change and counterexamples

Preparation now passes its existing target lock and exact checkout lease into
`run_release_notes_stage`. Its nested `lf --batch release-notes` receives both
held descriptors. The real CLI and Codex harness already preserve descriptors
whose close-on-exec flag was cleared at that boundary; neither needs a new
launcher, ownership record or ambient process-role switch. Standalone notes
continue to supply no release capability.

The new built-CLI regression first reproduced a competing tag while the notes
provider survived both its release controller and nested CLI. The contender
returned `Ok("v0.9.2")` before inheritance was added. After inheritance, that case
passed, but a failed notes launcher exposed premature deletion of the JSON
context: the provider could no longer read its input after the controller exited,
and the expected completion marker never appeared.

The stage now retains its exact bounded JSON through the existing runtime prompt
writer. Each invocation uses a unique name, preserving earlier input across
retries. That writer also keeps runtime prompts out of Git. Checkout removal
remains governed by the existing lease; it removes these inputs with the checkout
once surviving children finish. Standalone notes retain inputs with their other
runtime prompts. Notes validation, previous-note restoration, provider-error
classification and deterministic fallback remain unchanged.

## Focused proof

`surviving_release_notes_provider_retains_target_checkout_and_context` exercises
release preparation → nested built CLI → Codex harness → simulated app-server.
The app-server completes the initialization/thread exchange and receives a real
`turn/start` request before waiting at the barrier. Two cases cover:

- Killing the exact fixture-owned release controller and nested notes CLI while
  the provider remains alive.
- A notes launcher starting the real CLI, then exiting unsuccessfully while its
  CLI/provider descendants remain alive, allowing real controller cleanup to run.

Before opening the barrier, a competing release defers and ordinary checkout
removal fails. Afterward the provider reads the original context, verifies its
selected version through the test's retained JSON, and writes release notes.
Target access and checkout removal return after the holders exit. Both cases
retain the caller's HEAD, branch, byte-identical index, staged/unstaged/untracked
work, unpublished commit and previous release notes.

Git, bare origin, CLI dispatch, harness protocol, processes and OS locks are real.
GitHub and the provider executable are simulated. This proves the application's
provider launch boundary, not arbitrary vendor binaries forwarding descriptors,
actual model execution, hosted publication, installed scheduling, UI-host proof
or either configured automatic settlement.

## Review and remaining work

The simulated review found that forwarding locks alone left the context lifetime
shorter than the provider's lifetime. Reusing the runtime prompt writer repairs
that second failure without a separate context store or alternate notes policy.
The private callback configures the existing notes child; it cannot acquire
release authority. The target, checkout and cron attribution capabilities retain
their distinct scopes. Product settlement and prerequisite policy are untouched.

Task revocation/compensation and source/worktree mutation children remain open.
So do historical telemetry prerequisite associations, bounded current recovery,
dated repair ownership, closed-obligation continuation and configured acceptance.
The missing `agent_turns` scorecard blocker remains; the retained 36 failures
include the original 35. Required UI/public proof and two adjacent automatic
executions, at least one publishing without manual repair, remain mandatory.
This slice neither publishes the PR nor completes the Task.

## Validation

- Before inheritance: new regression failed on competing tag access (9.59s).
- With inheritance only: controller-death case passed; failed-launcher case
  failed waiting for provider completion after context cleanup (37.59s).
- With retained input: both cases passed (19.23s).
- Existing `release_tests release_notes` selection: six passed (5.34s), including
  fallback classes, missing CLI, stale version, bounded input and interrupted retry.
- Normal candidate preparation through tag/publication: passed (12.59s), with
  simulated external services.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed
  (Clippy 17.45s).

No affected-suite gate, full CI, installation/sync, cron trigger, production
publication, PM handoff, PR publication, landing or Task completion occurred.
