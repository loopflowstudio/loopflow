# LOO-611 open questions and assumptions

- Assumption: the 4 stream-only usage rows without a provider thread (Codex, October) are dropped by the migration rather than kept in a second shape; they cannot be placed on a native turn.
- Assumption: rentals run the same lf build as the Machine that imports their `machine_records`, so copied session events already have the facts-only shape; no conversion of copies.
- Assumption: `RecordedAttempt` provider records report no usage after the change (usage lives on native turns); the DTO keeps the field.
- Open (sequencing): if the migration exceeds 120 s on an unpruned clone of Jack's store, land after v0.14.16 publishes and its online prune has converted the store.
- Assumption: stream-only usage the migration inserts carries the observing capture's `captured_event`; the input readers join usage by `COALESCE(origin.captured_event, e.captured_event)`, since those turns have no native `started` row. Such a turn has no start, so it counts one evidence gap where the stream copy counted none; tokens and cost are unchanged.
- Assumption: a matched Claude turn takes one stream observation (final receipt, else latest) rather than the per-field maximum across observations; Claude's final receipt already carries the turn's totals.
- Assumption: Codex turns with both copies now read the native row only (the old reader preferred a final recorder receipt). Codex totals may shift slightly in the clone parity check; gate records the difference.
