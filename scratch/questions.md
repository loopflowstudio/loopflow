# Open questions and assumptions (LOO-364)

- **Primary is a Session attribute, not a pointer table** (assumed 2026-10-01).
  Unit 2 proposed `PrimarySession { scope, run_id, replacement_run_id }`. The
  landed model lets the AgentSession row carry `primary_scope`, and replacement
  is one transaction after the predecessor's provider stops, so no prepared
  successor id needs to survive between steps. Revisit only if a replacement
  must outlive a stop whose outcome stays unknown.
- **The Wave Session runs in the Wave binding's checkout** (the main checkout
  today). A dedicated scope checkout via `ensure_agent_worktree` with a local
  base is slice 6; until then design notes land in the shared checkout.
- **`serve-ask` launches primaries too.** The hidden command already serves any
  prepared row Session; it was not renamed.
- **`lf session ready` refuses in a primary.** Nothing waits on a primary, so
  readiness has no reader. `lf session complete ID` still completes one; the
  next `ensure` admits a fresh conversation.
- **`primary_scope` is not on the Session DTO yet.** It joins the wire shape and
  fixtures with its first Desktop consumer (LOO-353).
- **`lf session ensure` without `-w` means the repository** (assumed
  2026-10-01). The plan said `ensure --repo`; the flagless form is the smaller
  surface and the repository is the default scope everywhere else.
- **The repository Session is keyed by the canonical local checkout path**, the
  same value `agent_sessions.repo` already holds. A relocated checkout starts a
  new conversation.
- **CI repair identity includes failing check URLs.** A provider rerun of the
  same head is new evidence and can get its own repair. Left as shipped.
- Candidate instrument for the Wave: count of CI incidents per PR head with more
  than one `repair_session_id` across identities. It would show whether the
  URL-keyed identity lets reruns start repeat repairs; cheapest producer is a
  query over `ci_incidents` in `lf repo ci --json`. Proposal only.

Check (2026-10-01): `cargo test -p loopflow --lib primary::tests` 5 passed;
`cargo clippy -p loopflow --all-targets -- -D warnings` and `cargo fmt` clean.
Deferred to gate: affected suites, the regenerated `docs/lf-reference.md`
(entries were written by hand in the generator's format). Deferred to demo: a
real provider launch and `lf session connect` on a first-launch primary.
