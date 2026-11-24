# Choices made at kickoff, for review

- New event kind `rehoming_completion_requested` instead of a `children` field on
  `family_completion_requested`: released `lf` would settle a field-tagged rehome
  request by abandoning. Passed over: one kind with a field (cleaner, unsafe skew).
- iOS learns about children through a new `ViewContent.open_children` field so the
  swipe can ask before writing. Passed over: swipe always sends `children: null`
  and the refusal Notice offers the choice (one round trip wasted, destructive
  choice from a Notice button).
- Desktop default button is rehome (non-destructive); abandon is the destructive
  second button. Passed over: abandon-first as today.
- The live-child guard refuses the whole completion rather than abandoning the
  idle children and skipping the live one.
