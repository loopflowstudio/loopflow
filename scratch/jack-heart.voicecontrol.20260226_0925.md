# Voice Control — Stage 1: Push-to-Talk

Mic button in the WaveSessionView composer. WhisperKit transcribes on-device. Text streams into the composer field. Manual send.

## What to build

A `VoiceInputButton` next to the existing composer TextField, backed by a `VoiceInputService` that wraps WhisperKit for on-device speech-to-text.

## Data structures

```swift
// LoopflowCore/Services/VoiceInputService.swift

@Observable
@MainActor
final class VoiceInputService {
    enum State: Equatable { case idle, recording, transcribing }
    enum PermissionStatus { case notDetermined, granted, denied }

    private(set) var state: State = .idle
    private(set) var partialTranscript: String = ""
    private(set) var permissionStatus: PermissionStatus = .notDetermined

    /// Start recording. Requests mic permission on first call.
    func startRecording() async throws

    /// Stop recording and return final transcript.
    func stopRecording() async -> String

    /// Cancel recording without producing a transcript.
    func cancel()
}
```

```swift
// Concerto/Views/VoiceInputButton.swift

struct VoiceInputButton: View {
    @Bindable var voiceService: VoiceInputService
    let onTranscript: (String) -> Void
    // Tap to toggle record, or press-and-hold to record while held
}
```

## Key functions

- `VoiceInputService.startRecording()` — request mic permission, init WhisperKit pipeline (lazy, first use downloads tiny model), start audio capture, stream partial results to `partialTranscript`
- `VoiceInputService.stopRecording() -> String` — stop capture, finalize transcription, return text, reset state
- `VoiceInputService.cancel()` — stop capture, discard, reset
- `VoiceInputButton` — SF Symbol mic icon, three visual states (idle/recording/transcribing), tap-toggle and hold-to-record gestures, accessibility labels

## Composer integration

```swift
// In WaveSessionView, the composer HStack:
HStack(alignment: .bottom, spacing: Spacing.sm) {
    VoiceInputButton(voiceService: voiceService) { transcript in
        composerText += transcript
        focusedField = .composer
    }
    TextField("Message", text: $composerText, axis: .vertical)
        ...
    Button("End") { ... }
    Button("Send") { ... }
}
```

`VoiceInputService` lives as `@State` on `WaveSessionView`. Partial transcript shows as preview text below the composer while recording.

## Dependencies

Add to `Package.swift`:
```swift
.package(url: "https://github.com/argmaxinc/WhisperKit.git", from: "0.9.0")
```

Add `"WhisperKit"` to both `LoopflowCore` and `Concerto` target dependencies.

Add to `Concerto/Info.plist`:
```xml
<key>NSMicrophoneUsageDescription</key>
<string>Concerto uses the microphone for voice-to-text input.</string>
```

Add `com.apple.security.device.audio-input` entitlement.

## Constraints

- WhisperKit `tiny` model only (small download, fast inference)
- First-run model download must show progress, not block UI
- If mic permission denied, show inline notice with settings link
- All animations respect `reduceMotion`
- Hit targets: 44pt iOS, 24pt macOS minimum
- Audio capture stops on view disappear
- Partial transcript updates on MainActor

## Done when

- `swift test --package-path swift` passes
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` passes
- Manual: tap mic button → speak → see text in composer → edit → send
- Permission denied state shows helpful message
- Model downloads on first use with progress indicator
