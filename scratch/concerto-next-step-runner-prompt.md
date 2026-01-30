# Step Runner Prompt Passthrough

Wire the prompt text field in StepRunner through to wave execution.

## Problem

The `StepRunner` view has a prompt text field (`@State private var prompt: String`) that accepts user input, but this text is never passed to the execution layer. When users type "add rate limiting to auth endpoints" and hit Run, the agent never sees it.

Two execution paths exist, neither accepts a prompt:

1. **Interactive**: `sessionState.launchInteractiveSession(waveId:step:worktreePath:)` → terminal runs `lf <step>` without the prompt
2. **Auto**: `repoState.runWave(wave:flow:stimulus:)` → daemon runs flow without the prompt

## Approach

Pass prompt text as CLI arguments appended to the step content, following the existing `step_args` pattern in `gather_prompt_components()`.

The prompt flows through:
```
StepRunner → InteractiveSession → terminal command → lf CLI → step content
```

For interactive sessions, append prompt text as positional arguments to the `lf` command:
```bash
lf design "add rate limiting to auth endpoints"
```

The existing `step_args` processing in `context.py:891-903` appends plain args to step content:
```python
if plain_args:
    step_content = step_content.rstrip() + "\n\n" + " ".join(plain_args)
```

This means user prompt becomes part of the step's instructions—exactly what we want.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `--prompt` flag | New flag for lf CLI | Duplicates existing step_args behavior; more code for same result |
| Stdin/pipe | Pass prompt via stdin | Complicates terminal spawning; doesn't work with interactive mode |
| Clipboard hack | Copy prompt to clipboard, use `-c` | Destroys user's actual clipboard; implicit side effects |
| Environment variable | `LF_PROMPT=...` | Non-standard; hidden context; doesn't appear in logs |

## Key decisions

**Use positional args, not `--prompt` flag.**

The CLI already handles positional args after step name—they become `ctx.args` in Typer and get processed by `gather_prompt_components()`. No new CLI changes needed.

**Escape/quote the prompt properly.**

Shell escaping matters. The prompt may contain quotes, newlines, special characters. Swift side must shell-escape before building command string.

**Don't pass prompt for flows (auto mode).**

Flows run multiple steps autonomously. User prompt doesn't make sense mid-flow. For auto execution via daemon, the prompt field is informational only—wave direction/area provides the context.

**Following concerto-next principles:**

Per `04-improvise-ux.md`: "Run" button runs step on wave, shows output. The prompt field is labeled "Additional context (optional)"—it's extra guidance for the step, not a replacement for the step itself.

## Scope

**In scope:**
- `InteractiveSession` struct: add `prompt: String?` field
- `SessionState.launchInteractiveSession()`: add `prompt` parameter
- `StepRunner.runStep()`: pass prompt text for interactive execution
- `InteractiveSessionView`: build command with shell-escaped prompt
- Shell escaping utility function

**Out of scope:**
- Auto/daemon execution (flows don't use ad-hoc prompts)
- WaveService HTTP API changes
- Python CLI changes (existing step_args handles it)

## Implementation

### Swift: InteractiveSession

Add optional prompt field:

```swift
// swift/LoopflowCore/Models/Wave.swift
public struct InteractiveSession: Sendable, Identifiable {
    public let id: String
    public let waveId: String
    public let step: String
    public let worktreePath: String
    public let prompt: String?  // Add this
    public let startedAt: Date

    public init(
        id: String = UUID().uuidString,
        waveId: String,
        step: String,
        worktreePath: String,
        prompt: String? = nil,  // Add this
        startedAt: Date = Date()
    ) { ... }
}
```

### Swift: SessionState

Update launch method signature:

```swift
// swift/Concerto/State/SessionState.swift
func launchInteractiveSession(
    waveId: String,
    step: String,
    worktreePath: String,
    prompt: String? = nil  // Add this
) {
    interactiveSession = InteractiveSession(
        waveId: waveId,
        step: step,
        worktreePath: worktreePath,
        prompt: prompt
    )
}
```

### Swift: StepRunner

Pass the prompt:

```swift
// swift/Concerto/Views/Improvise/StepRunner.swift
private func runStep() {
    guard let path = wave.worktreePath else { return }

    let isInteractive = allSteps.first(where: { $0.name == selectedStep }) != nil

    if isInteractive {
        // Pass non-empty prompt, nil otherwise
        let promptArg = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        sessionState.launchInteractiveSession(
            waveId: wave.id,
            step: selectedStep,
            worktreePath: path,
            prompt: promptArg.isEmpty ? nil : promptArg
        )
    } else {
        // Auto mode unchanged
        ...
    }
}
```

### Swift: InteractiveSessionView

Build command with shell-escaped prompt:

```swift
// swift/Concerto/Views/InteractiveSessionView.swift
private var terminalContent: some View {
    let command = buildCommand()
    return GhosttyTerminalView(
        workingDirectory: session.worktreePath,
        command: command,
        sessionId: session.id,
        manager: ghosttyManager
    )
    .background(Color.black)
}

private func buildCommand() -> String {
    var cmd = "lf \(session.step)"
    if let prompt = session.prompt {
        cmd += " \(shellEscape(prompt))"
    }
    return cmd
}
```

### Swift: Shell escaping utility

```swift
// swift/LoopflowCore/Utilities/ShellEscape.swift
public func shellEscape(_ string: String) -> String {
    // For bash/zsh: wrap in single quotes, escape internal single quotes
    // 'foo' -> 'foo'
    // foo's -> 'foo'\''s'
    let escaped = string.replacingOccurrences(of: "'", with: "'\\''")
    return "'\(escaped)'"
}
```

## Done when

1. User types "add rate limiting to auth endpoints" in StepRunner prompt field
2. Clicks Run (design step)
3. Terminal shows: `lf design 'add rate limiting to auth endpoints'`
4. Agent's step content includes the prompt text at the end
5. Prompt with special characters (quotes, newlines, $vars) works correctly

Verify:
```bash
# In worktree directory
lf design 'add rate limiting to auth endpoints'
# Agent should receive step content + "\n\nadd rate limiting to auth endpoints"
```
