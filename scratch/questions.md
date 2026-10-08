# Open choices — LOO-406, 2026-10-07

The [design](explore-loopflow-s-own-store.md) owns scope, preservation evidence and
acceptance. The engineering choices below implement Jack Heart's accepted scope;
they are not additional decisions attributed to Jack. Follow-up product choices
do not block the single-machine lifecycle.

- **Deletion recovery (resolved engineering choice, 2026-10-07):** retain
  `task_issue_identities` solely for recovery and import only issues with established
  ownership. This preserves existing behavior within the accepted scope; the
  design owns the counterexample and required migration proof.
- **Import identity (2026-10-07):** the supposed existing deterministic mapper is
  absent. Reuse established mappings; otherwise mint and persist one UUID v4 with
  the unique provider mapping transactionally. Cross-machine convergence remains
  a synchronization follow-up.
- **Storage cut (2026-10-07):** keep durable identity on the existing Task/Project
  owner; nested plans carry optional `linear_id`, without duplicating durable IDs.
  Creation retries compare the retained original input, not edited current fields.
- **Public creation identity (2026-10-07 engineering finding):** the existing CLI
  hashes Wave/title/report as its provider marker. Local creation must retain a
  separate operation identity so an interrupted retry reuses its Task while an
  independent identical request creates another. The store receipt implements
  the latter distinction; the public recovery path remains to build, without a
  new product decision.
- **Local selector:** the resolver accepts `lf-<12 UUID hex digits>` and longer
  prefixes, rejecting ambiguity. Stored display labels currently use the full UUID;
  shortest-unique display remains part of the CLI/reader cutover. Full IDs remain
  authoritative. This does not change Jack's accepted scope.
- **Callback follow-up:** interpret host as origin. Resolve disconnect handling and
  whether X may access Linear directly after origin loss. The narrower proposal
  retains uncertain-write IDs and authored text for deliberate retry.
- **Git follow-up:** decide whether eventual visibility with semantic conflicts
  satisfies Jack's requirement to see new work without merges. Proposed timings
  are CLI-boundary attempts and Desktop five-second debounce/thirty-second cap
  and refresh while active, with no offline guarantee. Private-remote onboarding
  and configuration remain undesigned; namespaces alone provide no privacy.
- **Linear export follow-up:** prove duplicate-safe creation under lost responses
  and concurrent machines. Marker lookup and a write-once mapping are insufficient;
  provider-supported identity/readback and shared export ownership need validation.
