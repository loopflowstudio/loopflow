path: demo

The fix lives entirely in `scripts/install.py`'s promotion logic
(`_resolve_applications_dir` / `_promote` / wheel install), but its value is
not the diff — it's whether the deployed `lf` binary now resolves to 0.10.0
and reads the Linear roadmap instead of demanding `pm.asana_project`. The
scratch notes already contain a full before/after transcript (`command lf
--version`, `type -a lf`, `lf op pm show --wave architecture`) showing the
stale-shadow failure and the fixed behavior. Walking through that experience
proves the fix; reading the path-resolution code in isolation would not
demonstrate that the *active* app bundle actually got refreshed. No
provider-selection or lfd route-contract code changed, so there's no
algorithmic logic that needs a code walkthrough on its own merits.
