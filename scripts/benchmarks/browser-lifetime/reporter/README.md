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

Desktop builds use `-Dsentry=false`. The completed synthetic envelope comparison
was removed after selecting the signal journal and macOS reports.
