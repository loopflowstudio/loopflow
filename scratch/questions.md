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
- **Native turn delivery into a live interactive client is unproved** for Claude
  and OpenCode. It gates the outbox delivery slice only.
- Candidate instrument for the Wave: count of minute-schedule checks that found
  pending observations with no primary Session to receive them. It would show
  whether wakes are reaching a conversation; cheapest producer is the existing
  `lf task automation --json` reading. Proposal only.

Check (2026-10-01): `cargo test -p loopflow --lib primary::tests` 4 passed;
`cargo clippy -p loopflow --all-targets -- -D warnings` and `cargo fmt` clean.
Deferred to gate: affected suites, the regenerated `docs/lf-reference.md`
(entries were written by hand in the generator's format). Deferred to demo: a
real provider launch and `lf session connect` on a first-launch primary.
