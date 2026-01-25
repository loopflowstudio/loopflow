# Open Questions for Embedded Terminal (Ghostty)

## Answered

1. **Exact xcframework location** — `vendor/ghostty/macos/GhosttyKit.xcframework` (built by `zig build` with appropriate flags)

2. **PTY management** — libghostty handles PTY management internally. The `ghostty_surface_new` function spawns the shell process with PTY.

## Still Open

1. **CI complexity** — Is Zig 0.15.2 (development/HEAD) available in GitHub Actions? May need custom setup step.

2. **Config theming** — How to programmatically apply loopflow cream/slate colors to Ghostty? Options:
   - Pass colors via config at runtime
   - Use Ghostty's config file discovery
   - Apply custom config via `ghostty_config_load_file`

3. **Build dependency** — The xcframework must be built before the Swift project. Need to document the build sequence:
   ```bash
   # First: Build Ghostty (requires Zig)
   cd vendor/ghostty && zig build -Doptimize=ReleaseFast

   # Then: Build Concerto
   cd swift && xcodegen generate && xcodebuild
   ```

4. **Graceful fallback** — When GhosttyKit is not available (not built), the code compiles but shows a placeholder. Should we:
   - Display error message?
   - Auto-disable the feature flag?
   - Log warning on startup?
