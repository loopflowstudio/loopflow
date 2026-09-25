# Remaining native build boundary

After the Flow and Comments slices and their focused reviews settle, run the
existing resource-checked, supervised `loopflow` compile plan alone:

```python
from scripts.test import build_plan, run_plans
raise SystemExit(run_plans([p for p in build_plan([], False, {"loopflow"}) if p.run]))
```

Execute with `uv run python`; do not invoke a broader affected-suite plan merely
to obtain the Xcode compile. Preserve preflight failures and use only supported
resource recovery. This is the existing runner, not a preflight bypass.

Source inspection confirms `swift/project.yml` includes the entire `LoopflowMac`
directory except Info.plist/GhosttyResources, and all `LoopflowTests`. Newly added
Swift views therefore enter Xcode generation through existing globs. That source
fact does not establish fallback compilation; the compile above remains pending.
SwiftPM's focused native tests link Ghostty, whereas Xcode's app/test target
checks the terminal fallback. Neither substitutes for configured provider/demo
or human acceptance.
