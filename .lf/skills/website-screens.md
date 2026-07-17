Refresh the public Loopflow captures from the promoted app and the real product
Wave. Run exactly:

```bash
uv run python scripts/refresh_website_screens.py --publish
```

Do not stage mock data, edit the images, or substitute a fixture when live state
is unavailable. The script validates, captures, perceptually compares, writes
provenance, and publishes only meaningful changes. Report its result and stop.
