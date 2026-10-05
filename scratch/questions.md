# Remaining design choices — LOO-366

Jack Heart concluded review before the October 5 implementation attempt.
Accepted direction and proposed mechanics live in
[the design](keep-every-wave-ready-for.md); the earlier review notes are preserved
at `6dc8536fedce0852bd4931682c7ffb0a871cf17d:scratch/questions.md`.

## Implementation details still open

- Cached-name policy: [the indistinguishable histories](keep-every-wave-ready-for.md#mixed-representation-blocks-automatic-classification--october-5)
  require either one historical name-only exception with evidence retained, or
  explicit repair. The iteration direction retains equal-revision conflict protection;
  archiving the old body alone does not satisfy that constraint. Jack Heart has not
  selected the exception or a record-specific repair policy; dependent work remains.
- Chapter metadata representation and projection; historical cross-Wave inspection
  remains a proposed extension, while Wave-scoped chapter creation is requested.
- Existing skill/Flow composition and retained Task-candidate format between
  KR planning, chapter creation and separate Task admission.
- Designated fixture Waves and provider write authority for configured acceptance.

Jack Heart's October 5 shared-local ownership decision is resolved in
[the design](keep-every-wave-ready-for.md#shared-local-configuration-ownership--accepted-october-5),
including the Home-local path assumption. The design owns implementation status
and the remaining deletion list; the saved Flow retains its review boundary.
