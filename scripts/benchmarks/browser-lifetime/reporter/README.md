# Native reporter diagnostic

Build the pinned, patched Ghostty source in a private directory, separate from
`swift/.build/artifacts`, with Zig 0.16 and:

```sh
zig build -Doptimize=ReleaseFast -Demit-macos-app=false -Di18n=false \
  -Dsentry=false -Dxcframework-target=native -Demit-docs=false
uv run python scripts/benchmarks/browser-lifetime/reporter/run.py \
  --framework /private/diagnostic/source/macos/GhosttyKit.xcframework/macos-arm64
```

The framework argument is a native **slice**, including its Headers directory
and `libghostty-internal.a`. This command compiles a disposable executable with
Desktop's current `ExitSignals.swift`. It refuses libraries containing exported
Sentry/Breakpad symbols, initializes a terminal with external I/O (no shell or
provider), verifies clean shutdown, then faults a second child through an invalid
memory access. Both children deny network access and use private Homes and
configuration; neither launches Desktop. An explicit signal sent with `kill`
would bypass Mach exception handling and would not test the same behavior.

The receipt requires a terminal-ready marker, one matching signal record, and a
macOS `.ips` matching the child PID, newly linked Mach-O UUID and fault interval.
macOS redacts paths under Home, so a basename or `procPath` alone cannot identify
this executable. Evidence stays outside Git in the existing private incident
directory. The command kills and reaps its own child on interruption or timeout.
It installs nothing and changes no preferences, package pins or reporter policy.

The alternative experiment is
`desktop_exits::envelope_experiment::` in Rust's library test binary. Synthetic
fatal envelopes carry executable metadata plus minidump header, exception and
process-info streams. The parser seeks over attachments and reads neither thread
contexts nor stack/memory streams. A receipt names the envelope and embedded dump
attachment; it does not invent a separate extracted dump path. Ambiguity, missing
or torn evidence, a different executable, PID or process birth time, and crash
buckets outside the launch interval stay unknown. The candidate is test-only:
production `lf doctor` does not scan envelopes.

Integration would need the launch journal to record the executable and kernel
process creation time. Its current bundle/PID/start line cannot establish exact
process identity, and no production fallback guesses those missing fields.
The envelope parser currently targets sentry-native's explicit-length items;
unknown formats stay unknown. Sentry's [envelope specification](https://develop.sentry.dev/sdk/foundations/envelopes/)
and Breakpad's [metadata definitions](https://github.com/google/breakpad/blob/main/src/google_breakpad/common/minidump_format.h)
describe the formats. No real incident envelope or dump enters the tests.
