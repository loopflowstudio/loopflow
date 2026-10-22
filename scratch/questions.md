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
