# Open choices

## iOS GhosttyKit

- **One archive or two.** The framework now carries macOS, iOS and iOS
  Simulator libraries (about 270 MB + 2 × 134 MB before compression). Desktop's
  relink copy and libintl extraction keep only the macOS slice, so the app does
  not grow, but the download does. A separate iOS archive would keep Desktop's
  download as it is and add a second pin.
- **How the iOS project finds the published archive.** `swift/ios.yml` links a
  local path. Once published it could depend on the `GhosttyKit` binary target
  in `Package.swift` (one pin for both apps), or CI could fetch the zip into
  `swift/.build/local/`.
- **libintl on iOS.** The iOS slices statically combine GNU libintl
  (LGPL-2.1-or-later) as the macOS one does. Desktop ships the uncombined
  library, source and a relink script; the iOS app ships only notices. That
  needs settling before the app leaves development builds: package the same
  material, or build the iOS slices without i18n.
- **`ghostty_init` on iOS logs `error: NoHomeDir`** once at start. Nothing
  fails; Ghostty looks for a home directory for its own config and crash
  reports, which the phone app does not use.
