# LOO-625 open questions and assumptions

- Migration drops `waves.workflow`. This repository's Waves got `config.yaml` with
  their stored selections (infrastructure: code, product/growth: feature). Other
  repositories lose theirs on upgrade: manabot `intelligence` (iterate) and
  etude-studio `fantasia` (feature) — rerun `lf wave workflow set` there, which now
  writes their `config.yaml`. Assumed acceptable (no slow rollouts).
- `lf wave workflow set` writes `config.yaml` in the current checkout; the person
  commits it. A Wave directory missing from the checkout is created.
- Wave setup reports the Tasks Machine; a capacity default (pool/on-demand) shows as
  this Machine with source `wave`/`repo`, not the capacity name.
- Swift does not mirror `WaveDetailSnapshot.setup` (no UI uses it yet).
- Acceptance: one end-to-end `lf task run` proves a Wave's agent and account route;
  the second Wave and the no-Wave repository default are proven through the launch
  preview and `lf wave status`. Machine is proven through `lf wave status` only
  (test machines are local).
