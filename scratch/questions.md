# Open questions and assumptions

## Parent-to-child mutation inventory

The remaining control surfaces cannot be moved onto the Project capability as
part of LOO-227 without changing established Wave authority:

- Project pursuit may `task steer`, `interrupt`, `wait`, and `resume`. Resume is
  the only parked-Task relaunch mutation and now requires the immediate
  Project's exact capability. Wait is read-only; interrupt already refuses
  because there is no exact process owner.
- `task steer` is also an explicit Wave root override. Requiring the Project
  capability there would reject the Wave, which receives no equivalent exact
  capability today.
- `task run` dispatches new Work from PM ownership and is available to both
  Wave and Project pursuit. It is not an existing-child recovery mutation.
- Project `run`, `steer`, and `resume` likewise sit below a Wave control
  boundary that has no first-class capability lifecycle.
- Task `complete`, `recover`, and `abandon` are terminal or User-owned controls,
  not Project supervision. `recover` is explicitly documented as User-only,
  but that policy is not enforced by a shared sealed User boundary.

Material assumption: LOO-227 stays scoped to the reported parked-Task resume
surface. A follow-up that makes all Wave root overrides exact must design one
Wave-owned capability across Wave→Project and Wave→Task before tightening
shared commands such as `task steer`; adding only the Project check is a
regression, not prevention.
