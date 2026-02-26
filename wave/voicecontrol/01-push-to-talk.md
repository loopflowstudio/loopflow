# 01: Push-to-Talk

Mic button in the composer. Hold or toggle to record. WhisperKit transcribes on-device. Text streams into the composer field. Manual send.

## What to build

### WhisperKit dependency

Add WhisperKit to `Package.swift` and `project.yml`. Both macOS and iOS targets.

```swift
.package(url: "https://github.com/argmaxinc/WhisperKit.git", from: "0.9.0")
```

### Microphone permission

Add `NSMicrophoneUsageDescription` to `Concerto/Info.plist`:

```
Concerto uses the microphone for voice-to-text input.
```

Add `com.apple.security.device.audio-input` entitlement.

### VoiceInputService

New file: `LoopflowCore/Services/VoiceInputService.swift`

```swift
@Observable
@MainActor
final class VoiceInputService {
    enum State { case idle, recording, transcribing }

    private(set) var state: State = .idle
    private(set) var partialTranscript: String = ""

    func startRecording() async throws
    func stopRecording() async -> String
    func cancel()
}
```

Wraps WhisperKit's audio capture and transcription. On `startRecording()`:
1. Request microphone permission if not granted
2. Start audio capture via WhisperKit
3. Feed audio chunks to the transcriber
4. Update `partialTranscript` as results stream in

On `stopRecording()`:
1. Stop audio capture
2. Return final transcription
3. Clear `partialTranscript`, return to `.idle`

Model management: use WhisperKit's `tiny` model. Download on first use. Store in app support directory. Show a one-time "Downloading speech model..." indicator.

### VoiceInputButton

New file: `Concerto/Views/VoiceInputButton.swift`

```swift
struct VoiceInputButton: View {
    @Bindable var voiceService: VoiceInputService
    let onTranscript: (String) -> Void
}
```

Mic icon button placed left of the composer TextField. Two interaction modes:

- **Tap**: toggle recording on/off. Tap once to start, tap again to stop and insert text.
- **Hold**: press-and-hold to record, release to stop and insert text.

Visual states:
- **Idle**: `mic` SF Symbol, palette.textSecondary
- **Recording**: `mic.fill` SF Symbol, red/burgundy, subtle pulse animation (respects `reduceMotion`)
- **Transcribing**: `waveform` SF Symbol, processing indicator

Accessibility: label "Voice input", hint "Tap to start recording, tap again to stop."

### Composer integration

In `WaveSessionView`, the composer HStack becomes:

```
HStack {
    VoiceInputButton(voiceService: voiceService) { transcript in
        composerText += transcript
    }
    TextField("Message", ...)
    Button("End") { ... }
    Button("Send") { ... }
}
```

The transcript appends to `composerText` — user can review/edit before sending. Focus moves to the composer TextField after transcription completes.

### First-run permission flow

If microphone access is `.notDetermined`, the first tap on the mic button triggers the system permission dialog. If `.denied`, show a small inline notice: "Microphone access needed — open System Settings" with a button that opens the relevant settings pane.

## Constraints

- WhisperKit is the only new dependency
- All animations respect `reduceMotion`
- VoiceInputButton has minimum 44pt hit target on iOS, 24pt on macOS
- VoiceInputButton has `accessibilityLabel` and `accessibilityHint`
- Model download must not block the UI
- Audio capture must stop cleanly on view disappear

## Validation

- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- Manual: tap mic, speak, see text appear in composer, edit, send

## Done when

The mic button is in the composer. Tapping it records audio, transcription appears in the text field, and you can send it. WhisperKit tiny model downloads on first use. Permission flow is graceful.
