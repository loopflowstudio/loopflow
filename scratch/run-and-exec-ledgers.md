# Run and exec ledgers

## Problem

`lf runs` currently lists every `lf` process. Lookups such as `pm show`,
`task status`, and `task wait` crowd out the agent work, while the token and
context fields on those rows are meaningful only when a skill called a model.
The same process-grained list leaks into `lf status`.

## Model

- **Run**: one agent-backed skill launch. Owns skill/flow identity, model,
  context size, tokens, cost, and outcome.
- **Exec**: one `lf` process. Owns argv, parent process, and trace identity.
- **Flow**: orchestration around zero or more runs. A run carries its enclosing
  flow; a flow is not itself a model run.

## Surfaces

- `lf runs` lists skill launches across the machine.
- `lf status <wave>` filters the same run dataset to one Wave.
- `lf execs` preserves the process ledger used for plumbing diagnostics.
- `lf trace <exec-id>` reconstructs the process tree containing that exec.

Cap after selecting the right grain. Active runs survive the cap.

## Proof

- A lookup process appears in `lf execs` and not in `lf runs` or status.
- A skill launch appears in both `lf runs` and its Wave status with its model,
  token, cost, and supplied-context totals.
- Swift decodes the shared run DTO used by both run surfaces.
