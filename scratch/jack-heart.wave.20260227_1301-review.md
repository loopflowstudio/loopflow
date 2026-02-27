# Review: Rename "Start a wave" → "Ride a wave"

## What was implemented

Renamed user-facing "start a wave" language to "ride a wave" across the entire surface: README, Python CLI help text, tmux layout comments, and the Concerto macOS landing view.

The Swift view was renamed from `StartWaveView` to `CatchWaveView`, with internal methods updated (`startWave` → `catchWave`, `canStartWave` → `canCatchWave`). The user-facing text reads "Ride a wave" / "Ride wave".

Behavioral change: the empty-name guard was removed from the button — users can now submit without a name and get an auto-generated one (handled by `RepoState.createWaveInternal`, which already had this logic).

## Key choices

- **"Ride" for users, "Catch" for code.** The struct is `CatchWaveView` (surfer metaphor — catching a wave), while the button and heading say "Ride a wave" (simpler verb for the UI). Internal Rust functions (`start_wave_run`) kept their names — they describe execution mechanics, not user-facing language.
- **Auto-name generation stays in RepoState.** `RepoState.createWaveInternal` already generates names via `NameGenerator` when given an empty string. The view passes the trimmed name through rather than duplicating the logic (and `NameGenerator` is internal to `LoopflowCore`).

## How it fits together

Pure rename across 6 files. No new types, no API changes, no state changes. The only behavioral difference is the button is no longer disabled when the text field is empty.

## Risks and bottlenecks

None. This is a cosmetic rename with one small UX improvement (auto-name).

## What's not included

- Internal Rust/proto references (`start_wave_run`, `RunWave | Start wave execution`) are unchanged — these describe execution mechanics, not user-facing terminology.
- iOS views don't reference `StartWaveView` / `CatchWaveView` — no changes needed there.

## Gate fix

`CatchWaveView` referenced `NameGenerator` directly, but `NameGenerator` is internal to the `LoopflowCore` module. Removed the direct reference since `RepoState.createWaveInternal` already handles empty-name generation. Swift tests pass (213/213).
