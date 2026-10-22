# LOO-526 open questions

For Jack Heart, raised in the Task conversation on 2026-10-21. Each has the
default the design takes if unanswered.

## Q1. Final records at the hard deadline: push from the pod, or pull from the laptop?

The brief says the worker pushes its final Session and Process records to its
owning Machine before the guardian deletes. A pod has no route to the laptop:
no inbound SSH, and the laptop is not on a network the pod can see. Options:

- **Pull** (design default): the laptop copies on every pass and at the soft
  deadline; the guardian's hard-deadline delete only loses records nobody could
  reach in the whole window.
- **Push over Tailscale**: the pod joins the tailnet with an ephemeral auth key
  (one more Doppler secret, tailscale installed at boot) and runs
  `lf --machine <laptop> history import` before the guardian fires. Real push,
  more moving parts, and the laptop must be awake.
- **Push to R2**: the pod writes its export to a bucket (Doppler has R2 keys)
  and the laptop imports it on its next pass. Etude's storage pattern in
  miniature; records survive any laptop absence.

## Q2. Routing surface

Design default: `lf machine use <pool> --task X` resolves a pool name where a
Machine label goes. Alternatives: `lf pool run <pool> X` or `lf task run X
--pool code`. The default keeps one placement command and makes "set a Wave's
default to a pool" a one-line follow-up.

## Q3. Where lf gets RUNPOD_API_KEY

Design default: from the environment (`doppler run -p etude -c prd -- lf pool …`),
matching how Linear's connect is documented. Alternative: the pool stores a
Doppler reference (`doppler:etude/prd/RUNPOD_API_KEY`) and lf shells out to
`doppler secrets get … --plain` when it rents, so plain `lf pool rent` works.
Moving the key into the `loopflow` Doppler project is a separate choice.

## Q4. Pool machine hard deadline

Design default: `--hours 4` on `lf pool add`, soft deadline 15 minutes earlier.
Pool-made machines usually end by drain long before; the deadline is the bound
on a laptop that disappears.

## Q5. Demo before landing

The pod runs this branch's Linux build (Docker amd64, scp) until the release
with `lf pool` is published; after that the installer suffices. Acceptable for
the demo, as in LOO-554?
