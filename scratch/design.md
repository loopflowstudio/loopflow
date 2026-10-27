# LOO-624: RunPod, AWS and Fly.io on the rental boundary

Jack, 2026-10-26: "I think we should do RunPod, AWS, and DigitalOcean too",
(DigitalOcean dropped by Jack after the concept review, 2026-10-26),
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
| Fly.io | REST, no SDK | machine name in a per-account app | dedicated IPv4 TCP service on 22 | `auto_destroy` + init exits after N |

Images: Modal/Daytona build from `ubuntu:24.04` + TOOLS in the bridge. RunPod
and Fly take the same OCI image (built once, pushed to a registry); AWS boots stock Ubuntu and run TOOLS as cloud-init. Then `lf` installs over SSH as
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

So for Modal and Daytona the guardian is the provider's scheduler:
the same promise with nothing running in the box. A guardian is one of two kinds:

- **Exit**: the box ends when its main process does (Modal, Fly, AWS).
  The process is `lf-guardian <deadline> -- sshd -D`; the provider deletes the
  box (or for AWS, terminates it) once it exits.
- **Delete**: the box calls its provider's delete (RunPod).

One `guardian.py` (standard library, carried over from main), one entry per
provider in its table; `sandbox.rs` names each provider's guardian beside its
price. Daytona keeps its native TTL and no in-box guardian. Every guardian
gets the same unit tests carried over: the deadline stands, the budget only
shortens it, a clock set backwards buys nothing, delete retries until done.


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
AWS (`AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`, region).
The live demo per provider needs those set (`doppler secrets set NAME > /dev/null`).

## Concept review (2026-10-26)

The promise people rely on is "a rental ends by its deadline even if my laptop
dies"; the deadline ("until 18:30") is the one concept users see, and the
guardian is how a provider keeps it. DigitalOcean could not keep it (a
powered-off Droplet bills; self-delete needs an account-wide token; Machine-local
rentals mean a lost laptop leaves the Droplet billing), so Jack dropped
DigitalOcean. Every provider now keeps the printed deadline.

Decided (Jack, 2026-10-26), to build next:
- **Region stays in the environment** (`AWS_REGION`, `FLY_ORG`). The quote
  and `lf ondemand add` name the region they priced, so a us-east-1 price
  shown for another region is visibly an estimate.
- **`--gpu` combines with `--vcpus`/`--memory-gb`**: they are floors. The
  chosen GPU shape (RunPod GPU pod, EC2 GPU instance, Modal GPU sandbox) must
  meet them; if none does, the add or quote fails naming the provider, GPU
  and hardware. Never silently dropped.
- **`--spot`** wherever the provider sells interruptible capacity: AWS (spot,
  `MaxPrice` = the admitted price, so the budget holds) and RunPod
  (interruptible pods, bid = the admitted price). Daytona, Modal and Fly have
  none: `--spot` is refused naming the provider. An interrupted machine is
  found gone by reconcile like any ended rental; the Task's unpushed work on it
  is lost, which `lf ondemand add --spot` says once.

## Order

Stacks on LOO-527's `c5a0918783`; rebase as it moves, onto main once it lands.

Built (uncommitted, 2026-10-26): shared `guardian.py` (exit kind on Fly and
AWS, delete kind on RunPod) and its tests; RunPod, AWS and Fly in the bridge
with request-shape tests over mocked HTTP/boto3; `Provider::offer` (price and
the provider's machine type: RunPod flavor/GPU id, EC2 instance type, Modal
GPU); `--gpu` on RunPod, AWS and Modal; a refresh that finds a machine ended
still deletes it. DigitalOcean was built, then dropped (Jack).
Assumptions in `scratch/questions.md`.

Built (2026-10-26, after the concept review): `Hardware.spot` (draft
`rental_spot` backfills `false`), refused off AWS/RunPod; AWS spot with
`MaxPrice` = admitted price, RunPod `interruptible`; RunPod GPU pods send
`minVCPUPerGPU`/`minRAMPerGPU`, EC2 GPU shapes must hold the hardware; add,
quote and rent name AWS's region and that it priced us-east-1.

Remaining:
- Live run per provider (needs `RUNPOD_API_KEY`, AWS keys + region in Doppler);
  README rental row only after each passes.
- LOO-527 is PR #1704; publish stacked on it when Jack asks.

Check: `cargo test -p loopflow --lib rental` 29 passed, clippy and ruff clean, bridge+guardian pytest 19 passed (2026-10-26).
