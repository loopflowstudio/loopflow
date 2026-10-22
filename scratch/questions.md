# LOO-526 open questions

Raised in the Task conversation on 2026-10-21. Each has the default the design
takes if unanswered.

## Decided by Jack Heart, 2026-10-21

- **Q1 push vs pull.** Pull: the laptop copies records on every pass and at the
  soft deadline; the guardian's hard-deadline delete only loses records nobody
  could reach in the whole window. Push over Tailscale or R2 stays an option
  later.
- **Q2 routing surface.** `lf task run X --machine M | --pool P | --rent R`,
  following Etude's `--pool`/`--rent` split of 2026-10-21 (#23).

## Open

### Q3. Where lf gets RUNPOD_API_KEY

Default: from the environment (`doppler run -p etude -c prd -- lf …`), matching
how Linear's connect is documented. Alternative: the on-demand stores a Doppler
reference (`doppler:etude/prd/RUNPOD_API_KEY`) and lf shells out to `doppler
secrets get … --plain` when it rents, so plain `lf ondemand rent` works. Moving
the key into the `loopflow` Doppler project is a separate choice.

### Q4. Hard deadline for machines rented for a Task

Default: `--hours 4` on `lf ondemand add`, soft deadline 15 minutes earlier.
Such machines usually end by drain long before; the deadline bounds a laptop
that disappears.

### Q5. Demo before landing

The pod runs this branch's Linux build (Docker amd64, scp) until the release
with `lf ondemand` is published; after that the installer suffices. Acceptable,
as in LOO-554?

### Q6. Pools of owned machines in this Task

Default: yes, minimally. `lf pool add/off/on/list/remove`, `lf machine add
--pool/--cost-usd/--expires-at`, and `--pool` on `lf task run`. Expiry removes
a machine from placement and nothing else. Alternative: on-demands and `--rent`
only now, pools in their own Task.

## Assumptions taken in implement (2026-10-21)

- **Q3** took the default: `RUNPOD_API_KEY` from the environment.
- **Q6** took the default: pools of owned machines are in this Task.
- **`lf task run --machine M`** was not added: the root `--machine` flag already
  forwards a command, and `lf machine use M --task X` hands off. Placement adds
  only `--pool`, `--rent`, `--quote`.
- **`--rent` refuses under `-b` before renting**: a fresh machine needs a
  browser approval per login, so a headless run would pay for a machine it
  cannot use. `lf ondemand rent` + `lf machine connect` is the headless path.
- **By-hand rentals** (`lf ondemand rent`) are added as Machines only; they are
  not signed in, cloned or bound until a Task is placed there.
- **Unpushed-work check** scans `~/src/*/` on the rented machine (lf's default
  repo root there) and counts `lf`/`claude` processes as live work.
- **SSH user** on pods is `lf`, routed as `ssh://lf@IP:PORT`; the first contact
  accepts the fresh host key (`StrictHostKeyChecking=accept-new`).
