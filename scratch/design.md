# LOO-624: RunPod, AWS, DigitalOcean and Fly.io on the rental boundary

Jack, 2026-10-26: "I think we should do RunPod, AWS, and DigitalOcean too",
"maybe fly.io too?" Stacks on LOO-527 (`../loopflow.rental-machines`,
uncommitted as of 2026-10-26 16:10, no PR).

## Demo

```bash
doppler run -- lf ondemand add gpu --provider runpod --gpu A40 --max 1
doppler run -- lf ondemand add box --provider aws --vcpus 4 --memory-gb 16 --spot
doppler run -- lf task run LOO-xxx --rent box       # runs there end to end
lf ondemand list                                    # box-1 · aws · running · LOO-xxx · $0.03 so far
# land; after the grace, reconcile deletes box-1. kill -9 lf mid-run: next reconcile finds box-1 by name and deletes it.
```

## Shape

LOO-527's boundary is one bridge, `rental/sandbox.py`: `<provider> <op>` with
JSON in/out for create, get, named, delete; `sandbox.rs` owns `Provider`, the
price table and the Machine route. Each new provider is one `_<provider>(op,
request)` function, one `Provider` variant, its price rows, and request-shape
tests with mocked HTTP. No second abstraction.

| Provider | SDK pin in the bridge | Name lookup | SSH route | Hard lifetime |
|---|---|---|---|---|
| RunPod | REST, no SDK | pod name | public IP + mapped 22 | none native |
| AWS EC2 | `boto3` | tag `loopflow:name` | public IP, key in user-data | `shutdown -h +N` with terminate-on-shutdown |
| DigitalOcean | REST, no SDK | droplet name (+ `loopflow` tag) | public IP, SSH key | none native; poweroff still bills |
| Fly.io | REST, no SDK | machine name in a per-account app | dedicated IPv4 TCP service on 22 | `auto_destroy` + init exits after N |

Images: Modal/Daytona build from `ubuntu:24.04` + TOOLS in the bridge. RunPod
and Fly take the same OCI image (built once, pushed to a registry); AWS and DO
boot stock Ubuntu and run TOOLS as cloud-init. Then `lf` installs over SSH as
in LOO-527.

## Guardians (Jack, 2026-10-26: "bring back guardians, make them shared concept")

A guardian holds a rental's deadline when no lf is alive. Admission already
computes the deadline once: min(lifetime, budget ÷ admitted price)
(`rental/mod.rs`, LOO-527). Reconcile is the courteous deleter (drain, grace,
history copied up); the guardian is the backstop that never needs lf, a
laptop or the network. It never extends a deadline.

What a guardian is on each provider:

| Provider | Guardian | Credential on the box |
|---|---|---|
| Modal | the sandbox's main process: `timeout` ends it; Modal's own `timeout` is the second layer | none |
| Daytona | Daytona's `ttl_minutes`; in-box it can only stop the container, which isn't deleting it | none |
| Fly | main process exits at the deadline, `auto_destroy` deletes the Machine | none |
| AWS | `shutdown -h` at the deadline, `InstanceInitiatedShutdownBehavior=terminate`; spot `MaxPrice` = admitted price keeps the budget true | none |
| RunPod | `guardian.py` deletes the pod via the pod-scoped `RUNPOD_API_KEY` RunPod injects, and re-reads the charged price to shorten the deadline | pod-scoped, injected by RunPod |
| DigitalOcean | power-off still bills; self-delete needs an account token (custom scope `droplet:delete`, still every Droplet) | **account-wide** |

So for Modal and Daytona the guardian is the provider's scheduler:
the same promise with nothing running in the box. A guardian is one of two kinds:

- **Exit**: the box ends when its main process does (Modal, Fly, AWS).
  The process is `lf-guardian <deadline> -- sshd -D`; the provider deletes the
  box (or for AWS, terminates it) once it exits.
- **Delete**: the box calls its provider's delete (RunPod, DigitalOcean).

One `guardian.py` (standard library, carried over from main), one entry per
provider in its table; `sandbox.rs` names each provider's guardian beside its
price. Daytona keeps its native TTL and no in-box guardian. Every guardian
gets the same unit tests carried over: the deadline stands, the budget only
shortens it, a clock set backwards buys nothing, delete retries until done.

Open (Jack, 2026-10-26: undecided): DigitalOcean's guardian puts a token on
the box that can delete every Droplet. Until decided, DO ships with no guardian.

## Decided

- Fly: reachable over IPv4 (Jack reversed IPv6-only after learning this Mac
  has no global IPv6 route). Fly's proxy balances across an app's Machines,
  so one dedicated IPv4 per app (~$2/mo, unverified) means one app per rental
  or one shared app with a port per Machine; pick when Fly is built.
- GPUs: `--gpu` on RunPod, AWS and Modal; other providers reject it, naming
  the provider (Jack). Modal takes it on the same `Sandbox.create(gpu=...)`
  (modal 1.6.2), so GPU boxes keep the sandbox's SSH tunnel and guardian; the
  Function API would be a second path to the same box.

## Credentials

Doppler has `DAYTONA_API_KEY` and `FLY_API_TOKEN`. Missing: `RUNPOD_API_KEY`,
AWS (`AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`, region), `DIGITALOCEAN_TOKEN`
(optional `DIGITALOCEAN_REGION`, default `nyc3`).
The live demo per provider needs those set (`doppler secrets set NAME > /dev/null`).

## Concept review (2026-10-26, proposed; nothing below is accepted yet)

The product promise people rely on is "a rental ends by its deadline even if
my laptop dies." Users see one concept for it: the deadline printed as
"until 18:30". The guardian is how a provider keeps it, not something users
learn. Every provider keeps it except DigitalOcean, yet `lf ondemand list`,
`lf machine end` and the scheduled check (`rental.rs` `check`: "its provider
deletes it at the deadline either way") say it for DigitalOcean too.

Counterexample: a DigitalOcean rental, laptop lost. Rentals are Machine-local
and the scheduled check reaches a provider only with its key in the
environment, so no lf anywhere looks for the Droplet. It bills indefinitely.

Proposed: each `Provider` names who holds its deadline (provider, guardian, or
lf only), and every surface that prints a deadline says it truthfully. The
choice is what DigitalOcean is in this PR (question 1 in
`scratch/concept-review.html`).

Smaller, proposed:
- Region is ambient (`AWS_REGION`, `DIGITALOCEAN_REGION`, `FLY_ORG`), yet it
  sets price (EC2 rates are us-east-1 only) and placement. An on-demand could
  carry `--region` so two on-demands can differ and the quote is true.
- With `--gpu` on RunPod, `--vcpus`/`--memory-gb` are silently dropped. Treat
  them as floors the chosen GPU shape must meet, or refuse them with `--gpu`.

## Order

Stacks on LOO-527's `c5a0918783`; rebase as it moves, onto main once it lands.

Built (uncommitted, 2026-10-26): shared `guardian.py` (exit kind on Fly and
AWS, delete kind on RunPod) and its tests; RunPod, AWS and Fly in the bridge
with request-shape tests over mocked HTTP/boto3; `Provider::offer` (price and
the provider's machine type: RunPod flavor/GPU id, EC2 instance type, Modal
GPU); `--gpu` on RunPod, AWS and Modal; a refresh that finds a machine ended
still deletes it. DigitalOcean (Jack, 2026-10-26): create/get/delete over
REST in the bridge with request-shape tests, dedicated-CPU sizes, no guardian,
out of the README and `docs/lf.md` until the guardian credential is decided.
Assumptions in `scratch/questions.md`.

Remaining:
- Live run per provider (needs `RUNPOD_API_KEY`, AWS keys + region,
  `DIGITALOCEAN_TOKEN` in Doppler);
  README rental row only after each passes.
- Still open with Jack: DigitalOcean's guardian credential; AWS spot. Modal
  Functions are not needed yet (Jack); `--gpu` stays `Sandbox.create(gpu=...)`.
- No publication until LOO-527 has a PR to stack on.

Check: `cargo test -p loopflow --lib rental` 26 passed, clippy clean after the `offer`/`find_gpu` compress (2026-10-26); bridge pytest unchanged (13 passed).
