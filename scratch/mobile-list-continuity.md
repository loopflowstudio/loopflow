# Keep Tasks current through Session navigation

Physical phone QA showed Back inserting Updating saved Tasks and Last updated above unchanged rows, then removing them. The list's disappearance currently closes its subscription. Keep the one subscription with the connected foreground workspace across Task/Session navigation; backgrounding or removing that workspace still closes it. Do not hide stale/error state or add a refresh timer.

Verify the production view's disappearance callbacks retain live readings until the workspace closes, then exercise native Back/reopen. Check background/foreground and stream failure recovery with the existing focused tests.

Source73a855ba5 keeps updates with the NavigationStack's foreground lifetime. Focused native checks passed:8 Task-update tests,1 device-protection skip,0 failures in1.636s; chosen-Session return40.550s and landscape Find/input25.277s. The landscape test now stops immediately if Find remains visible after Close, retaining a screenshot; this is a diagnostic improvement, not an established fix for the earlier CI missed action. Complete runner4679 exited0 and removed its Simulator.

A proposed hosted-parent ViewInspector probe failed with State/Environment warnings and UIKit InvalidTransition; it did not reliably exercise navigation and was removed, with production unchanged during that trial. Its log is retained separately. Do not count that trial as a reliable before/after regression.

Signed phone build/signature passed. Artifact /Users/jack/.lf-machine/demos/mobile-718-live-73a855ba5/LoopflowiOS.app is retained, not installed. At16:01 local iPhone Mirroring reported “iPhone in Use” and “Lock your iPhone to connect.” Physical Back/reopen verification of this repair remains pending. The prior installedf8cac5b95 app and its connection were left intact. Logs:reviewroot/list-continuity. No live local check/build handles; no publication yet.

At16:04 local, Connect resumed Mirroring. Installed73a855ba5 normally, reopened through Spotlight and read38 real Working Tasks. Searched “native iOS”, opened this Session passively and returned: same filtered row position with no Updating/Last updated insertion. Reopened, searched “fixed build” in local Find, saw Match found and the highlighted phrase, then Done dismissed Find on its first click. No terminal input or provider launch. Screenshots are in the conversation. This establishes portrait Find through Mirroring, not the CI landscape/software-keyboard case.

The CI accessibility snapshot measured Done at37×44 points. Give it the same44-point minimum width as adjacent terminal controls; exercise its layout and actual center hit-test in the existing Find regression. This improves the touch target without claiming it caused the missed CI action.

Sourcefd3d0d098: native Find layout/hit-test passed0.154s; signed device build and strict signature passed. Installed16:09 local, reopened normally through Spotlight; saved Machine restored with38 real Tasks. Phone left at Tasks, no Session keyboard control. Artifact /Users/jack/.lf-machine/demos/mobile-718-live-fd3d0d098/LoopflowiOS.app. All build/test/install handles finished; no publication. Updated PR copy:reviewroot/list-continuity/pr-copy.json.

Next live UX question: new observer attachments to this desktop-sized terminal show older output at the top of an oversized grid, with Latest already visible. Inspect whether initial attachment should position the phone viewport at the latest output without moving it again while someone reads older text. Screenshots16:05–16:06 show this behavior; no implementation change or cause established yet.
