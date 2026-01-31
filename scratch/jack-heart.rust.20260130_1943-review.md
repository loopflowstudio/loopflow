# Review: Concerto wave detail polish + Rust data backend design

## What was implemented
- Reworked the Concerto wave detail panel to introduce a dedicated action bar, a commits section, and an abandon confirmation flow.
- Added `OutlineButtonStyle` for secondary actions (Stop, Abandon).
- Renamed sidebar language from “Waves” to “Agents” in empty states and header copy.
- Replaced the Stage 5 roadmap stub with a detailed Rust data backend design doc in `scratch/rust-data-backend.md` and tracked open questions in `scratch/questions.md`.

## Key choices
- **Action bar separation:** Actions now live in a fixed bar above the scroll area to keep them visible and distinct from progress/content sections.
- **Commits panel vs. files-only:** Commits are shown as a first-class section, using `git log main..HEAD` for quick per-wave history.
- **Destructive flow:** Abandoning a wave is gated behind a confirmation dialog to reduce accidental deletes.

## How it fits together
- `WaveDetailPanel` composes `actionBar`, `commitsSection`, and `changedFilesSection` inside the conduct view.
- `WorktreeService` provides commit data via `getCommits`, and `WaveDetailPanel` orchestrates loading + display.
- UI styling remains consistent through the shared palette and new outline button style.

## Risks and bottlenecks
- **Abandon semantics:** The UI deletes the wave record but does not explicitly address worktree cleanup—this is an unresolved product decision.
- **Commit history baseline:** Commits are calculated against `main` by default; if waves can be based on other branches, the history may be misleading.
- **Ghostty warnings in tests:** Swift build emits linker warnings from Ghostty’s static library; this is pre-existing but still noisy.

## What’s not included
- No change to worktree cleanup behavior on abandon.
- No PR badge or direct PR link in the new action bar.
- No adjustment to commit baseline selection (still `main`).
