# AppleScript-Triggered Screenshot Capture

Make Maestro scriptable so agents can trigger self-screenshots and get the path back.

## What to build

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

## Key functions

### CaptureService.swift fix

Current code uses `NSApp.keyWindow` which is nil when Maestro isn't frontmost:

```swift
// Current (broken when not frontmost)
guard let window = NSApp.keyWindow else { ... }

// Fixed (works regardless of focus)
guard let window = NSApp.windows.first(where: { $0.isVisible }) else { ... }
```

### Script command handler (new file: ScriptCommands.swift)

```swift
import Cocoa

class CaptureScreenshotCommand: NSScriptCommand {
    override func performDefaultImplementation() -> Any? {
        guard let repoPath = directParameter as? String else {
            scriptErrorNumber = NSRequiredArgumentsMissingScriptError
            scriptErrorString = "Repository path required"
            return nil
        }

        let repoURL = URL(fileURLWithPath: repoPath)
        let service = CaptureService()

        do {
            let screenshotURL = try service.captureWindow(repoRoot: repoURL)
            return screenshotURL.path
        } catch {
            scriptErrorNumber = NSInternalScriptError
            scriptErrorString = error.localizedDescription
            return nil
        }
    }
}
```

### Maestro.sdef command binding

```xml
<command name="capture screenshot" code="MSTRcapt" description="Capture Maestro window">
    <cocoa class="Maestro.CaptureScreenshotCommand"/>
    <direct-parameter type="text" description="Repository path"/>
    <result type="text" description="Screenshot path"/>
</command>
```

## Constraints

- **Maestro must be running with a visible window**: Capture fails if no window exists
- **Screen Recording permission**: Maestro needs permission in System Settings
- **Maestro-only**: No CLI fallback. If Maestro isn't available, capture isn't available.

## UI changes

None - this is infrastructure for agent automation.

## Done when

```bash
# From terminal (simulating what agent would do)
osascript -e 'tell application "Maestro" to capture screenshot to "/Users/jack/src/loopflow"'
# → /Users/jack/src/loopflow/.design/screenshots/maestro-20260116-150000.png

# Verify file exists
ls /Users/jack/src/loopflow/.design/screenshots/
# → maestro-20260116-150000.png

# Agent reads the screenshot
lf ux-research -x .design/screenshots/
```

## Files to change

```
Maestro/Maestro/Maestro.sdef                    # New: AppleScript dictionary
Maestro/Maestro/Info.plist                      # Add NSAppleScriptEnabled, OSAScriptingDefinition
Maestro/Maestro/ScriptCommands.swift            # New: command handler
Maestro/Maestro/Services/CaptureService.swift   # Fix to work when not frontmost
```

## Not in scope

- CLI fallback (`lfops screenshot`) - Maestro is the only driver
- Multiple window capture
- Capture delay/timing options
