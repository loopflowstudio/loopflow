# Open questions / assumptions

- Apple VAD implementation currently uses partial-transcript inactivity timing to segment utterances. It does **not yet** implement energy-based onset/offset detection with pre-roll ring buffering from the design doc. This keeps the first draft functional and testable, but Apple-path onset clipping/false-positive tuning will need a follow-up pass.
