# Open questions (LOO-1109)

Choices made with nobody present. Each is reversible; the plan carries the
chosen side.

1. **"Open it in rex" opens the OS default browser.** Rex has no browser
   block (`rex block types --json`: onboarding, `terminal.shell`, tunnel) and
   `agent-browser` is not installed here. Passed over: launching a headed
   `agent-browser` from `lf page open`. Which browser did "an agent-browser
   split" mean?
2. **`data-lf` runs on click after an inline confirm, not on `⌘↩`.** The
   queue is words for the agent; a `data-lf` button is one operation now.
   Passed over: batching operations into the submit, which needs
   partial-failure rules.
3. **Any `Operation` is accepted over the loopback, no allowlist**, behind a
   per-server token, a `Host` check and a same-origin `Origin` check. Follows
   "its basically another proxy for the lf machine server interface"; the
   earlier "1b" (narrow allowlist) is passed over.
4. **In the browser `⌘↩` steers at once** (prototype 2a). The reply "open to
   either 2b" was not clear enough to build typing into the agent's unsent
   input.
5. **The token lives in memory**, so a server restart invalidates open pages.
   Passed over: a token file in the lf home.
6. **`lf open --file` no longer exists** (#2034). The plan adds
   `lf page open` instead of restoring it.
7. **`tour` is untouched**: it writes `scratch/tour.yaml`, not a page.
8. **Source is `scratch/<name>.page.html`, output `scratch/<name>.html`.**
