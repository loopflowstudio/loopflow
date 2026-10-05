# Remaining design choices — LOO-366

Jack Heart concluded review before the October 5 implementation attempt.
Accepted direction and proposed mechanics live in
[the design](keep-every-wave-ready-for.md); the earlier review notes are preserved
at `6dc8536fedce0852bd4931682c7ffb0a871cf17d:scratch/questions.md`.

## Ownership resolved — Jack Heart, October 5

Linear comment `f092d63a-a152-4920-af81-d676a576f694` selects one shared local
Wave configuration across checkouts. The design now gives the Project binding
one Home-local file keyed by Wave ID; checkout copies and recovery receipts
cannot select the current Project. Publication/synchronization is no longer an
open prerequisite. Exact-ID validation, content/work preservation, planned KRs
for optional chapters and separate Task admission remain accepted.

Implementation assumption: `<Home>/waves/<WaveId>/config.yaml` is the shared
file location. Jack selected shared local ownership, not this exact spelling.
Other Wave policy stays with its existing owner. The saved Flow retains its
review boundary; substantial implementation remains in the design's five slices.

## Implementation details still open

- Chapter metadata representation and projection; historical cross-Wave inspection
  remains a proposed extension, while Wave-scoped chapter creation is requested.
- Existing skill/Flow composition and retained Task-candidate format between
  KR planning, chapter creation and separate Task admission.
- Designated fixture Waves and provider write authority for configured acceptance.

No provider mutation, reset, execution change or configured proof has occurred.
