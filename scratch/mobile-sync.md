# Mobile current state · November10

Task LOO-718, PR1812, checkout /Users/jack/src/loopflow.mobile. No merge, Auto-Ship or Task-completion authorization. Current source HEADabecd3ce0 merges upstream454f768944 after fixes97d2d10b9 (batch creation awaits original Task-run carrier) and c09d0f418 (visible unexpected connection errors/server acceptance diagnostics). Published head e0f6d4c19; publication next. Previous goal turn is progress, not a blocked turn. No live local handles; old CI watcher60090 finished.

## Checks

CI42892358001 on e0f6d4c19 finished: native iOS/Mac, Rust library, installation and other gates passed; one integration failure exposed batch creation waiting on its first Graph attempt while its original run retried. Reproduced locally, fixed in97d2d10b9. Receipt replay creates no new run. Current merged-tree142tests passed247.750s (creation, store events, transport_tests, two_people), fmtcheck/Clippy passed37.60s. Logs: /Users/jack/.lf-machine/reviews/mobile-718-20261110/creation-ci/. New CI needed after publication; no additional local test pass without changed inputs/failure.

Before the latest store merge: real Codex native acceptance passed26.57s, real Claude40.35s; edits, service restart, passive return and context-dependent replies retained provider processes. This is Simulator proof, not phone proof. Claude’s first initial TLS handshake failed at5.159s with URLSession-1200/-9816 and CancellationError; cause remains unknown. Subsequent run retained server diagnostics and passed, not a diagnosis. See mobile-provider-acceptance.md and review/{codex,claude}-acceptance. Earlier November4 experimental batching deadline failures also remain unexplained; those /tmp logs are gone.

## Real connection

Protocol5 Desktop/CLI bundle installed via normal installer; Rust source14d9259f2, packagingec3716a. Last checked ready, mobile ready, no error. Endpoint https://jacks-macbook-pro.tail0eda02.ts.net:53747. Installed Machine predates upstream bounded_identity_triggers migration; no new install or migration performed after sync. iOS protocol/model unchanged by that upstream merge. Never fake Tailscale identity on the real Machine.

Current signed phone artifact: /Users/jack/.lf-machine/demos/mobile-718-live-c09d0f418/LoopflowiOS.app; signature verified, build.json/QR/connect.html alongside. NOT installed/launched. Device mobile heart, A7DD4746-FBC3-5016-889F-3E33FF3E98DD, team2V3M244HF2, bundle com.loopflow.ios.dev. First install refused locked phone; second failed Apple device tunnel timeout after passcodeRequired=false. Latest read-only devicectl query: paired, localNetwork, tunnel disconnected, lastConnectionDate2026-11-10T19:56:00Z. No new install attempt. Needs reachable awake phone, then install/launch WITHOUT fixture, connect same-owner Tailscale, scan QR. Unlock question pending, no reply. No phone camera/typing/cellular acceptance.

Normal app no longer launches with --fixture. Creation keeps recovery until destination appears. Address editing hides the old Connect action and keeps Continue above keyboard. Native pairing7passed after connection-error fix; signed build passed. No installed iOS CLI, embedded Tailscale, custom relay, hub or new retention authority.

Keep personal demos/history/drafts untouched. November9 demo used FixtureTerminal and executed no commands. Source/authored notes are archived under review root. Before publication, lf pr publish stages untracked scratch in this non-persistent worktree: commit notes then lf scratch clear, publish with prepared copy, restore local notes afterward. Do not repeatedly restart CI with more polish commits; verify the next published head and address concrete failures.
