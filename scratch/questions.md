# Remaining design choices — LOO-366

Jack Heart concluded review before the October 5 implementation attempt.
Accepted direction and proposed mechanics live in
[the design](keep-every-wave-ready-for.md); the earlier review notes are preserved
at `6dc8536fedce0852bd4931682c7ffb0a871cf17d:scratch/questions.md`.

## Implementation details still open

- Preservation fence: explicit Started/worker claims omit unbound conversations
  associated by checkout. The revised design proposes existing checkout admission
  plus the shared Task-work reader, including missing-checkout/subdirectory and
  out-of-checkout binding paths. Prove both orderings and failed-reset re-entry
  before cutting over selection; historical binding must remain usable.
  Existing-issue registration can add a checkout after rotation collects its
  locks and associate earlier Sessions without a Session write. Stabilize the
  Task/checkout population too; generic Task updates can relocate it. The design
  records the entry points and reentrant-lock constraint. This is unresolved
  implementation mechanics, not a request to change Jack Heart's product policy.
- Chapter metadata representation and projection; historical cross-Wave inspection
  remains a proposed extension, while Wave-scoped chapter creation is requested.
- Existing skill/Flow composition and retained Task-candidate format between
  KR planning, chapter creation and separate Task admission.
- Designated fixture Waves and provider write authority for configured acceptance.

Jack Heart's October 5 shared-local ownership decision is resolved in
[the design](keep-every-wave-ready-for.md#shared-local-configuration-ownership--accepted-october-5),
including the Home-local path assumption. The design owns implementation status
and the remaining deletion list; the saved Flow retains its review boundary.

Jack Heart approved the bounded historical name-only cutover on October 5; the design records its evidence and strict post-conversion boundary.
