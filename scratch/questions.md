# Open Questions

- The branch design doc in `scratch/chords-data-model.md` currently describes a `Wave` struct + `WaveKind` approach, but implementation followed the wave-area docs and shipped the `Wave` enum (`Voice | Chord`) model. Confirm whether the design doc should be updated to match implementation before merge.
