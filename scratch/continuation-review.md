# LOO-416 operator review — October 17

Disposition: resolved by typed `reconnect` dispatch and separate explicit recovery.
`WorkspaceSessionLifecycleTests` covers the production buttons, ended/failed
attachments and canceled preparation; focused Swift checks pass. Original finding
and control guidance remain below as evidence.

Resolve before publication: in the current implementation, SessionPaneView.openButton
labels an attach-only record “Reconnect”/“Try again” but routes every .open to
SessionsStore.resume (generic connect). Retry/reconnect must remain attach-only;
explicit history recovery/native transition needs distinct action semantics, not
string-label dispatch. Ended/no-owner records must also retain a usable explicit
recovery action instead of only disabled Reconnect. Prove the production action/store
route, including occupied viewer, failed attach and known-ended execution.
This finding was saved as steer cc08dd6f-e960-4928-ba19-505649273a04; delivery is pending.

The inherited LF_BIN points to released 0.14.3 while the shared store is now 0.14.4.
For control commands use `env LF_BIN=/Users/jack/.lf/bin/lf-f95826d4549b6ee89f74e1d7cd0c9a2fdbe31a017a6b1e358d6858f67bc7bf43 lf ...`.
Changing argv alone can repin through the inherited LF_BIN. The existing release
matches the database. The original repository locator is now rebound to its same
repo_30115cfb3a8ecb51494082029703a1fa identity; no upgrade was performed here.
Candidate binaries and schema drafts belong only in disposable test homes.
The currently running Flow is older: inspect its actual result before recovering
any failed next step, never start competing execution.
