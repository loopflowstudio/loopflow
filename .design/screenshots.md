# AppleScript-Triggered Screenshot Capture

Maestro is now scriptable - agents can trigger self-screenshots via AppleScript and get the path back.

## What it does

Agents running in auto mode execute `osascript` to tell Maestro to capture its own window. Maestro returns the screenshot path.

## The problem

UX prompts need screenshots of Maestro's UI. Agents in auto mode are headless - they can't click buttons or use keyboard shortcuts.

The fix: the agent triggers Maestro via AppleScript. Maestro captures itself, saves to `.design/screenshots/`, and returns the path.

## User workflow

Agent runs:
```bash
osascript -e 'tell application "Maestro" to capture screenshot to "/Users/jack/src/loopflow"'
# → /Users/jack/src/loopflow/.design/screenshots/maestro-20260116-150000.png
```

Agent then reads the screenshot at that exact path.

## Data structures

### Maestro.sdef (new)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE dictionary SYSTEM "file://localhost/System/Library/DTDs/sdef.dtd">
<dictionary title="Maestro Terminology">
    <suite name="Maestro Suite" code="MSTR" description="Maestro scripting commands">
        <command name="capture screenshot" code="MSTRcapt" description="Capture Maestro window screenshot">
            <direct-parameter type="text" description="Repository path for saving screenshot"/>
            <result type="text" description="Path to saved screenshot"/>
        </command>
    </suite>
</dictionary>
```

### Info.plist additions

```xml
<key>NSAppleScriptEnabled</key>
<true/>
<key>OSAScriptingDefinition</key>
<string>Maestro.sdef</string>
```

## Implementation

### CaptureService.swift

Added `captureWindow(repoRoot:)` method that finds any visible window (not just keyWindow), so capture works when Maestro isn't frontmost. Uses `screencapture -l <windowID>` CLI under the hood.

### ScriptCommands.swift (new)

AppleScript command handler that receives the repo path, calls CaptureService, and returns the screenshot path. Reports errors via AppleScript's scriptErrorNumber/scriptErrorString.

### Maestro.sdef (new)

AppleScript dictionary defining the `capture screenshot` command with the `MSTRcapt` code. Binds to `CaptureScreenshotCommand` class.

## Constraints

- **Maestro must be running with a visible window**: Capture fails if no window exists
- **Screen Recording permission**: Maestro needs permission in System Settings
- **Maestro-only**: No CLI fallback. If Maestro isn't available, capture isn't available.

## UI changes

None - this is infrastructure for agent automation.

## Files changed

```
Maestro/Maestro/Maestro.sdef                    # New: AppleScript dictionary
Maestro/Maestro/Info.plist                      # Added NSAppleScriptEnabled, OSAScriptingDefinition
Maestro/Maestro/ScriptCommands.swift            # New: command handler
Maestro/Maestro/Services/CaptureService.swift   # Added captureWindow() for non-frontmost capture
Maestro/Package.swift                           # Exclude .sdef from build
Maestro/dev                                     # Copy .sdef to app bundle
```

## Not in scope

- CLI fallback (`lfops screenshot`) - Maestro is the only driver
- Multiple window capture
- Capture delay/timing options
