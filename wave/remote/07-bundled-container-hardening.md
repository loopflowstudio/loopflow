# 07: Bundled Container Hardening

**Finish line:** Native fallback behavior intentional, documented, reflected in UX. Concerto UI test behavior stable or documented as known flake.

## What remains

Native fallback is implemented (`concerto.bundledDaemon.preferNativeMode` persisted via UserDefaults, Connection Settings shows toggle). Two items:

1. **Concerto UI test crash.** `ConcertoUITests-Runner open() failed errno=1` seen in `xcodebuild test -scheme Concerto`. Classify as a known external flake with evidence in TESTING.md, or fix.
2. **Native fallback documentation.** Document the native fallback behavior and `preferNativeMode` setting for operators.

## Out of scope

- Studio JWT rollout (covered by `04-studio-auth.md`)
- Remote host lane parity work (covered by `02-mac-mini-dogfood.md`)
- New API surface expansion (covered by `05-api-expansion.md`)
