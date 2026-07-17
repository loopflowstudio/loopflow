# Open questions

None blocking. Revision 2 of scratch/living-website.md scopes this round to
one human-driven update: `uv run python scripts/capture_screenshots.py`
writes PNGs and provenance sidecars into website/static/ from whatever
Loopflow is installed; a human reviews `git status` and commits.

The ladder back up, explicitly later:

- One command that installs the latest loopflow and runs the whole capture
  process.
- Automation of that command (install hook or schedule), then the Done Whens
  in scratch/living-website.md.
