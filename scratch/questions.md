# Implementation choices — LOO-412, 2026-10-08

Jack Heart selected host callbacks and a fresh `pursue` through publication for
another review. [The design](work-on-another-machine-name.md) supersedes the
adoption-only assumptions preserved at `0cd8e7f14:scratch/questions.md`.

- Host means the originating machine, including through nested remote dispatch.
  The worker's own machine/account registry and execution stay local.
- Local persistence with pending Linear sync is the host's ordinary LOO-406
  behavior. Losing that host connection is different: preserve input and report
  unavailable/unconfirmed, without worker-local creation or direct Linear fallback.
- No direct Linear fallback, automatic edit replay, terminal relay or detached
  work is selected here. These omissions do not weaken callback creation/read/
  edit/comment, exact mutation retry or socket cleanup acceptance.
- LOO-406's single planning model is being revised separately. Integrate a coherent
  committed API through Loopflow when needed; never consume its dirty checkout or
  resurrect its superseded personal/shared authority split. Record the actual
  dependency if its required operation is not yet available.
- Fresh worker identity comes from the host. Existing divergent machine-local IDs
  and histories survive via the smallest necessary origin association. Its exact
  storage shape is an engineering choice, not authority to renumber records.
