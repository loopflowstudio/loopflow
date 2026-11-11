# Open choices

## iOS GhosttyKit

Settled 2026-11-11. One archive with all slices, used by both apps (Jack Heart:
"this is fine for now"). GhosttyKit builds with `-Di18n=false` everywhere (Jack
Heart: "maybe we could just turn off i18n for everything right now?" / "no one is
using it"), so no libintl and no LGPL relink material. Sources ship only where a license
requires it, MPL-2.0 (Jack Heart: "only if fully legal though"). Published as
`GhosttyKit-a60e9e2-lf8.xcframework.zip`; both apps link it from `Package.swift`.

- **`ghostty_init` on iOS logs `error: NoHomeDir`** once at start. Nothing
  fails; Ghostty looks for a home directory for its own config and crash
  reports, which the phone app does not use.
