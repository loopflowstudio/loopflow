# Test foreground Machine announcements

```sh
xcrun swiftc -swift-version 5 -parse-as-library scratch/tailscale-discovery/Probe.swift -o /tmp/loopflow-discovery-probe
/tmp/loopflow-discovery-probe
```

The macOS listener exits after one valid offer or 15 seconds. It prints no address.
This is a socket experiment, not the Loopflow app or an authorization implementation.

For physical iOS testing, create an isolated SwiftUI iOS application target with
`Probe.swift` as its only Swift source and a distinct development bundle identifier.
Use normal development signing for the test device; do not reuse another app's
identifier. Add an `NSLocalNetworkUsageDescription` explaining that the experiment
receives Machine address announcements. Do not add a VPN extension or background
network mode. The source requires iOS 17 or later.

Sign into the installed Tailscale app on the test phone and keep the probe open.
On a separate Machine, run this from the repository root, substituting the CLI
path and the Machine's HTTPS name:

```sh
uv run python scratch/tailscale-discovery/announce.py --tailscale /path/to/tailscale --endpoint https://machine.example.ts.net --seconds 15
```

The sender uses the host's current peer list. It targets online, untagged iOS
devices owned by the same Tailscale user, sends once every three seconds, and
stops within 60 seconds. It does not change Tailscale settings. `--include-self`
adds the sending host for diagnostics only; success there is not physical iOS
acceptance. The probe never contacts an offered endpoint, so receipt does not
require an operational HTTPS server and does not prove one exists.

Record device OS and Tailscale versions, host/phone network, number of eligible
peers, receipt latency and whether incoming connections are enabled. Repeat on
cellular, after background/resume, after VPN reconnect and with a second Machine.
Do not change access policies to conceal a default-path failure; record it.

Remaining production work includes authenticated server verification, bounded
receiver connections, deduplication/expiry, IPv6, non-personal server ownership
and power/latency measurements. The experiment has none of those guarantees.
