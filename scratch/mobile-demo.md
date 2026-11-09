# Mobile interaction demo — November 9

Native Simulator app in offline fixture mode. No live Tailscale connection,
provider or Loopflow Session is launched. Sample Tasks and Session IDs come from
the bundled fixtures; edits live in the app's in-memory terminal peer. Closing
or relaunching it can lose a draft. No feedback has been recorded yet.

- History root / LF_HOME: `/Users/jack/.lf-machine/demo-history/mobile-718-20261109`
- Runtime root: `/Users/jack/.lf-machine/demos/mobile-718-20261109`
- LF_BIN: `/Users/jack/.lf-machine/demos/mobile-718-20261109/lf`
- Private Simulator set: history root `/devices`
- Simulator: `3D183D1C-4D0A-46D7-ACD9-0B69713A8DBF` (Loopflow Mobile 718)
- App: `com.loopflow.ios.dev`, copied whole into runtime root before installation.
- Status: running at the Task list; no input entered by the agent. No release marker. Keep both roots while input is unsettled.

Reopen the existing viewer without relaunching its app:

```sh
open -a Simulator --args -DeviceSetPath /Users/jack/.lf-machine/demo-history/mobile-718-20261109/devices -CurrentDeviceUDID 3D183D1C-4D0A-46D7-ACD9-0B69713A8DBF
```

The private history is the Simulator data under the history root. There is no
provider conversation to resume with lf. Terminal draft state exists only in
the running app; filesystem history cannot reconstruct a dead editor.

After the conversation settles or gives up every draft, verify the recorded app
PID/start identity and use this exact device/app teardown (never a process-name
kill). If input was given up, leave the app running instead:

```sh
LF_HOME=/Users/jack/.lf-machine/demo-history/mobile-718-20261109 LF_BIN=/Users/jack/.lf-machine/demos/mobile-718-20261109/lf xcrun simctl --set /Users/jack/.lf-machine/demo-history/mobile-718-20261109/devices terminate 3D183D1C-4D0A-46D7-ACD9-0B69713A8DBF com.loopflow.ios.dev
```

Confirm the app exited before shutting down this private device or removing its
runtime root. Never delete the history root. Do not treat an app exit or silence
as settled input.

## Launch record

```text
58553 20082 Mon Nov  9 11:45:31 2026     /Users/jack/.lf-machine/demo-history/mobile-718-20261109/devices/3D183D1C-4D0A-46D7-ACD9-0B69713A8DBF/data/Containers/Bundle/Application/4CED7EDE-DC56-4B9E-A46A-6358E212D6F5/LoopflowiOS.app/LoopflowiOS
```

Simulator UI verified the three sample Tasks and “Offline fixture” label.
No reaction has been recorded. The app stays open for editing.

`xcodebuild build -project swift/LoopflowMobile.xcodeproj -scheme LoopflowiOS -destination "generic/platform=iOS Simulator" -derivedDataPath swift/.build/ios -skipPackagePluginValidation CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=-`: passed after replacing the remaining `viewPlan` call with `desktopViewGet`. Live connection review remains for a later demo.

## Live connection attempt — November 9

Jack Heart: “ok, seems ike the session works, lets connect it to my actual machine now”

The sample app remains open; this does not settle any draft. The live Machine
and server have not been changed.

- Installed `lf server status`: running, revision 2; installed `lf` has no
  `server mobile` subcommand and its server refuses `mobile.status`.
- Tailscale is running from `/Applications/Tailscale 2.app`; host MagicDNS and
  HTTPS are enabled. `/usr/local/bin/tailscale` points at a missing app path.
- The shared Work read returned seven repository plans. The branch's full
  Machine read failed: the live database lacks unmerged migration
  `mobile_device_admission`. Normal installation cannot apply this branch
  migration to the main Machine; do not bypass that check.
- Simulator shares the Mac's Tailscale node; remote admission requires a
  distinct owner device. Do not substitute fixture identity for live data.
- Paired iPhone `mobile heart` is listed as available. Its first developer
  services query timed out. A question about using that phone is pending.
- Device build: `xcodebuild build -project swift/LoopflowMobile.xcodeproj
  -scheme LoopflowiOS -destination 'generic/platform=iOS' -derivedDataPath
  swift/.build/ios-device -skipPackagePluginValidation
  DEVELOPMENT_TEAM=2V3M244HF2` passed. No provisioning updates requested.
- Signed app copied to
  `/Users/jack/.lf-machine/demos/mobile-718-live-20261109/LoopflowiOS.app`;
  `codesign --verify --deep --strict` passed. Not installed or launched.

Next: get the mobile server and its migration into an installable build, then
install it through the ordinary installation path. Pair the phone with the host's
real address and verify real Task and Session access. No server replacement,
main-database migration, live credentials or connection were simulated.

Revision-3 device build, November 9: the same signed build command passed after
the shared protocol update. Whole app copied to
`/Users/jack/.lf-machine/demos/mobile-718-live-rev3-20261109/LoopflowiOS.app`;
`codesign --verify --deep --strict` passed. Use this matching build for the next
phone installation. It is not installed or launched. The real Machine's
installation prerequisites and the pending phone choice remain unchanged.
