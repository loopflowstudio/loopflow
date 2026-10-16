Loopflow's app and GhosttyKit packaging previously omitted third-party notices and source materials. About Loopflow now opens an offline, selectable and searchable license viewer. One reviewed manifest and the locked Cargo runtime closure generate its inventory; changes to dependency pins or bundled resources invalidate stale notices.

Packaging now includes full notices, pinned source archives, the application source/build inputs, uncombined libintl libraries for both architectures, and a tested rebuild/relink recipe. App assembly and future GhosttyKit archive creation reject missing or mismatched materials before signing/publication. Symbols Nerd Font retains its component attribution and OFL terms; font bytes and Loopflow's own license are unchanged.

Validation: focused Python packaging/release/dev tests passed (including missing-source and archive mismatch cases), both Swift license tests and the app build passed, and the real app and compressed framework archive verified all 281 material files. A full release rebuild from packaged sources replaced libintl, relinked the Swift app, built its CLI and produced an ad-hoc-signed application. Python lint passed. The full platform/Xcode matrix runs in CI.

Source materials add approximately 217 MiB before distribution compression. Existing remote GhosttyKit objects remain immutable; this change packages the current app and future archives without publishing a new GhosttyKit binary. No follow-up Task is needed for the accepted scope.

Closes LOO-466.
