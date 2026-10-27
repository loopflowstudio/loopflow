# LOO-624 repair: Linear over OAuth on rentals, Fly removed, AWS proven

Status: accepted direction (Jack Heart, 2026-10-26/27); this plan covers the
work left after #1707 merged. A new repair PR carries it. #1707 is not the
completion record.

## Problem

#1707 (in v0.14.17) ships rentals on Daytona, Modal, RunPod, AWS and Fly. Three
things are still wrong for someone renting a machine for a Task:

1. A rented machine in a Linear-planned repository has no Linear access, so
   `lf task run --rent` cannot sync or move the Task from there.
2. Fly is shipped but cannot do the work: about 4.5 GB of free disk cannot hold
   a Rust build. Jack dropped it (2026-10-27).
3. No Task has run end to end on AWS.

## Decisions (Jack Heart)

- 2026-10-26: guardians are shared; the deadline is the user-facing concept.
  `--gpu` works on RunPod, AWS and Modal (`Sandbox.create(gpu=)`), with no
  Modal Functions. Region stays in the environment and the quote names it.
  `--gpu` combines with `--vcpus`/`--memory-gb` as floors. `--spot` works on
  AWS and RunPod and is refused elsewhere. DigitalOcean is dropped.
- 2026-10-27 14:01: "dont use linear api keys, use oauth". The API-key
  commit `45c94905fd` is reverted.
- 2026-10-27: Fly is removed: the provider, the bridge, its tests and its
  docs. The website's Fly deploy (`FLY_API_TOKEN`, `deploy_website`) stays.
- Live provider runs are a demo, not CI.

## Chosen approach

**Linear on a rental: the laptop refreshes and hands over only an access
token.** Linear rotates the refresh token every time it is used, so sharing
the grant would let a machine rotate the laptop's grant away. Each prepare
calls `ops::linear::renewed_planning_token`, which forces a refresh under the
existing refresh lock. It checks that the token's viewer is the laptop's Linear
login, then installs `{access_token, expires_at}` on the machine as an OAuth
token with no refresh token. There is no browser step and no per-machine OAuth
grant. Linear's access token lasts 24 h (observed live 2026-10-27), and a
rental's window is capped at `rental::MAX_HOURS` = 24 h. A token refreshed at
prepare therefore outlives the machine. A test pins that comparison, so raising
`MAX_HOURS` forces a decision.

**Fly stops renting; its shipped records stay.** `_fly`, its pricing, its
tests and its doc rows go. Jack's decision removes the provider, not the cost
history 0.14.17 wrote, and `lf ondemand remove` already promises "its rentals'
history stays". So `Provider::Fly` remains only to read those rows: hidden from
`--provider` (`#[value(skip)]`), and every quote and bridge call refuses it
("Fly no longer rents"). Draft `retire_fly_rentals.sql` marks a Fly rental
still live as `lost` (lf can no longer ask Fly; its guardian ends it at the
deadline) and clears a scope default naming a Fly on-demand. Nothing is
deleted. Anyone who rented there frees a leftover app with
`fly apps list | grep lf-rent-` and `fly apps destroy`.

**Brought up to main (done: 2979f7d26f merges 22b3801967, 8fd458a1ef merges
f7200e4138 without touching rental code).** Rental prose now
lives in `docs/ops.md` and `docs/architecture/machines.md` (`docs/lf.md` is
gone on main); the Fly removal follows main's `lf ops` command layout. The
only remaining Fly references are `Provider::Fly` as a retired reader and its
tests.

## Delete — do not maintain

- `Provider::Fly`, `FLY_IPV4_MICROS`, `_fly`/`_fly_app`/`_fly_org`, the `FLY_*`
  bridge constants, the Fly tests in `python/tests/test_rental_bridge.py` and
  `rental/tests.rs`, and the `FLY_RENT_TOKEN` docs. Done in the worktree.
- `account::fresh_linear_login` (per-machine Chrome OAuth) and the API-key
  path: `LINEAR_API_KEY` in `api_key_env_name`, the `lin_api_` bare-header
  branch in `LinearClient`, and `connect linear --api-key`. Done in the
  worktree.

## Forbidden outcomes

- Forwarding Linear's refresh token, or a second grant per machine.
- Tolerating unknown provider strings at read time; Fly is a known, retired one.
- Deleting shipped rental or cost history.
- A Linear API-key fallback.

## Absent and error states

- Laptop not connected to Linear: preparing the rental fails, naming
  `lf account connect linear`, as it does today for a missing planning token.
- The refreshed token belongs to a different viewer: the token is discarded
  and preparing fails. No machine receives it.
- Access token expired on the machine: Linear calls there fail with
  Linear's 401, and the laptop's grant is untouched.

## Exclusions

DigitalOcean, Modal Functions and GPU templates beyond `--gpu` are out of scope.
Closing the README's provider row waits on the AWS demo; it names Daytona,
Modal, RunPod and AWS only.

## Demo (live, not CI)

1. With Jack's AWS SSO session (temporary keys exported from it, nothing stored
   in Doppler), run `lf ondemand add aws1 --provider aws …`.
2. Run `lf -b task run <LOO-644 task> --rent aws1`. The Task runs on EC2,
   Linear works there from the forwarded access token, and
   `lf ondemand list` shows cost so far.
3. After completion and grace, the instance is terminated. Confirm with
   `aws ec2 describe-instances --filters Name=tag:loopflow-rental,...`.
4. Clean up afterwards: delete only this Task's `lf-rent-*` resources, and
   close or delete Linear issue LOO-644, which exists only for this demo.

## Done when

- A home holding 0.14.17 Fly rows opens; `lf ondemand list` shows them as
  history, none live, and nothing rents on Fly.
- `prepare` binds the Task's own repository identity, so a fresh home first
  used from a worktree rents without hand repair
  (`a_worktree_checkout_binds_its_tasks_repository_on_a_fresh_home`).
- A rented machine in a Linear-planned repository receives a fresh,
  viewer-checked access token and no refresh token.
- The AWS demo above completes and leaves no running instance.

## Acceptance at gate (headless)

- `cargo test -p loopflow machine_credentials rental linear < /dev/null`
  (inherited `LF_*` unset) passes. This includes
  `linear_installs_an_access_token_without_its_refresh_grant` and a migration
  test that seeds the released schema with a `fly` ondemand and rental and
  expects both reads to succeed.
- `uv run pytest python/tests/test_rental_bridge.py python/tests/test_guardian.py` passes.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` are clean.
- `UPDATE_CLI_REFERENCE=1 … reference_matches_the_compiled_cli` produces no diff after regeneration.

## AWS proof (2026-10-27, done)

Run from this checkout with the branch builds: `LF_HOME=~/.lf-qa2`,
`LF_RENT_LINUX_BIN=target/linux/release/lf`, AWS keys exported from Jack's SSO
session, `AWS_REGION=us-east-1`, on-demand `aws1` (4 vCPU / 16 GB, $0.2016/h,
cap $0.81, 4 h). `lf -b task run LOO-644 --rent aws1 -a claude`:

- EC2 m7i.xlarge in us-east-1; GitHub, Claude and Linear connected; clone over
  gh, bind, claim on the machine.
- Claude ran implement → compress → sync → realign → concept-review there;
  `cargo test -p loopflow --lib rental` passed on EC2 (32).
- `lf task status LOO-644` on the rental read Linear through the forwarded
  access token; `lf ondemand list` on the laptop showed cost so far.
- `pr-publish` then opened PR #1739 from the machine: the laptop-side
  `lf task interrupt` saw no live run (the run lived on the rental) and came
  too late. LOO-644's brief said not to publish, so #1739 was closed unmerged
  with its branch kept. The route stopped by itself at the `demo` stage.
- `lf machine end aws1-1` gave it back after $0.047 with 8 Sessions and 25
  Processes copied home; no instance left pending or running. About $0.10
  across six attempts, all terminated.
- LOO-644 disposition: deferred, owned on the laptop in `~/.lf-qa2`; its
  Linear issue and branch stay for Jack to close or keep.
- Not proven: automatic give-back after Task completion plus grace, which the
  original acceptance names. The Task stopped at a stage, and `lf machine end`
  proves only cleanup. Unit tests cover the path; the live condition stays
  open, with no further rental spending authorized here.

The proof found four defects, fixed in 29955657aa: the bridge's region, the
remote Git identity, Claude's inference scope, and dev builds on a
disposable remote home. f434845ed9 then drops the "`lf machine end` gives it
back" hint from a failed clone (prepare already gives the machine back) and
drops `boot_abandoned`'s machine-id check, which its only caller (`reconcile`)
already makes; behavior is unchanged.

## Branch recovery (2026-10-27, done)

The Task records `jack/loo-624-rent-from-runpod-aws-digitalocean-and-flyio-too-on-the-same`,
which #1707 merged at its tip `9a67dda1a6` (squash `df6b79d5ea`, in HEAD). The
repair was built on `jack/loo-624-fly-rent-token`, so `resolve_task_pr_base`
refused after sync. With Jack's authorization the repair was merged onto the
recorded branch as `3d50383aca` (parents `9a67dda1a6`, `2979f7d26f`; tree
identical to the repair tip). Recovery refs: `recovery/loo-624-fly-rent-token`,
`recovery/loo-624-pr1707-tip`. Neither branch exists on origin; nothing was
force-pushed. The repair PR publishes from the recorded branch.

## Remaining work, in order

1. Gate: clippy, fmt, CLI reference regeneration (post-merge), affected suites (`-j2`).
2. Publish a new repair PR. Root owns patch release and install.

Open with Jack: the live give-back after Task completion plus grace (see AWS
proof). The choices are another paid AWS run that reaches `end`, or accepting
unit coverage and dropping it from "Done when". No rental spending is
authorized for it now.

## Concept review (2026-10-27, headless)

Implementation matches the accepted model: Fly is a retired provider that
history can name but nothing can rent (`#[value(skip)]`, every quote and
credential check refuses with one message); Linear reaches a rental as a
short-lived access token only; `prepare` binds the Task's repository. No
clearer name or merged concept is implied by Jack's decisions, and no new
product choice surfaced. The live give-back question above is an acceptance
and spending decision, not a product-shape one, so it does not hold the gate.

## Check results

- `cargo test -j2 -p loopflow --lib -- rental machine_credentials fly_rows linear_installs` (env cleared): 50 passed at 2979f7d26f; `-- rental boot`: 37 passed at compress; the f7200e4138 merge touches no rental code. Gate owns clippy, fmt, CLI reference and suites.
