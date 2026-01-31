# Data Structures

## Wave

```swift
struct Wave {
    let id: WaveId
    var name: String
    var area: [String]
    var direction: [String]
    var flow: String
    var stimulus: Stimulus
    var status: WaveStatus
    // ... existing fields
}

enum WaveStatus {
    case idle
    case running(step: String, progress: StepProgress)
    case waiting(step: String, reason: WaitReason)
    case completed
    case error(message: String)
}

enum WaitReason {
    case interactive  // needs human input
    case rateLimit    // API limit hit
    case conflict     // merge conflict
    case approval     // waiting for PR approval
}

struct StepProgress {
    let current: Int
    let total: Int
    let output: AsyncStream<String>?
}
```

## Key Functions

```swift
// Wave lifecycle
func createWave(name: String, area: [String], direction: [String]?) async throws -> Wave
func connectToWave(_ waveId: WaveId) async throws -> Session
func setStimulus(_ waveId: WaveId, stimulus: Stimulus) async throws
func landWave(_ waveId: WaveId) async throws

// Running steps
func runStep(_ waveId: WaveId, step: String, prompt: String?) async throws -> StepRun
func continueStep(_ stepRunId: StepRunId) async throws  // the "Continue" button

// Dashboard
func listWaves() -> [Wave]
func listWavesNeedingAttention() -> [Wave]

// Cross-platform
func subscribeToNotifications() -> AsyncStream<WaveNotification>
```

## Prompt Delivery

**Current:** Prompts are passed as CLI positional args (`lf design 'add rate limiting'`). The `lf` CLI appends them to step content via the existing `step_args` machinery.

**Future:** For very long prompts, deliver via file to keep terminal commands clean:

```
.lf/history/
  2025-01-30T17:30:00-design.md
  2025-01-30T17:45:12-implement.md
```

The file approach creates traceable history and keeps giant prompts out of Claude's conversation context. The `.lf/history/` directory is gitignored.

## Constraints

1. **Must work with both Python and Rust lfd** — abstract the transport
2. **Mobile executes remotely** — same experience, agents run on your Mac
3. **Local = no auth, remote = Loopflow account** — simple split
4. **Waves always exist** — no "ephemeral exploration," wave is created when you start working
5. **Notifications must work cross-platform** — APNS for mobile, system notifications for macOS
6. **"Continue" button must be obvious** — not buried in terminal, always accessible
