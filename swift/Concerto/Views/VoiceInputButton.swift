import SwiftUI
import LoopflowCore

struct VoiceInputButton: View {
    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @Bindable var voiceService: VoiceInputService
    let onTranscript: (String) -> Void

    @State private var ignoreNextTap = false
    @State private var isPulsing = false
    @State private var isBreathing = false

    private var buttonSize: CGFloat { platformVoiceButtonSize }

    private var iconName: String {
        switch voiceService.state {
        case .idle:
            return "mic"
        case .listening:
            return "mic.badge.waveform"
        case .recording:
            return "mic.fill"
        case .transcribing:
            return "waveform"
        }
    }

    private var iconColor: Color {
        switch voiceService.state {
        case .idle, .listening:
            return palette.textSecondary
        case .recording:
            return palette.accent
        case .transcribing:
            return .statusInfo
        }
    }

    private var backgroundColor: Color {
        switch voiceService.state {
        case .idle, .listening:
            return palette.surfaceMuted
        case .recording:
            return palette.accent.opacity(0.15)
        case .transcribing:
            return Color.statusInfo.opacity(0.14)
        }
    }

    private var accessibilityHint: String {
        switch voiceService.state {
        case .idle:
            return "Tap to start recording. Long press to start hands-free listening."
        case .listening:
            return "Tap to stop hands-free listening."
        case .recording:
            if voiceService.inputMode == .voiceActivityDetection {
                return "Tap to cancel this utterance and continue listening."
            }
            return "Tap to stop recording."
        case .transcribing:
            return "Transcribing your audio."
        }
    }

    private var scaleEffectValue: CGFloat {
        guard !reduceMotion else { return 1 }

        switch voiceService.state {
        case .recording:
            return isPulsing ? 1.08 : 1
        case .listening:
            return isBreathing ? 1.04 : 1
        case .idle, .transcribing:
            return 1
        }
    }

    var body: some View {
        Group {
            if voiceService.inputMode == .voiceActivityDetection {
                button
                    .contextMenu {
                        sensitivityActions

                        Divider()

                        Button("Stop listening") {
                            voiceService.stopListening()
                        }
                    }
            } else {
                button
            }
        }
        .onAppear {
            voiceService.setTranscriptHandler(onTranscript)
        }
        .onDisappear {
            voiceService.setTranscriptHandler(nil)
        }
        .onChange(of: voiceService.state, initial: true) { _, _ in
            updateAnimationState()
        }
    }

    private var button: some View {
        Button(action: handleTap) {
            ZStack {
                Circle()
                    .fill(backgroundColor)

                symbol
            }
            .frame(width: buttonSize, height: buttonSize)
        }
        .buttonStyle(.plain)
        .scaleEffect(scaleEffectValue)
        .animation(DesignAnimation.standard(reduceMotion), value: voiceService.state)
        .animation(DesignAnimation.standard(reduceMotion), value: isPulsing)
        .animation(DesignAnimation.standard(reduceMotion), value: isBreathing)
        .contentShape(Circle())
        .simultaneousGesture(
            LongPressGesture(minimumDuration: 0.45)
                .onEnded { _ in
                    handleLongPress()
                }
        )
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Voice input")
        .accessibilityHint(accessibilityHint)
        .accessibilityAddTraits(.isButton)
    }

    @ViewBuilder
    private var symbol: some View {
        if voiceService.state == .listening {
            Image(systemName: iconName)
                .font(Typography.body())
                .symbolRenderingMode(.palette)
                .foregroundStyle(palette.textSecondary, palette.accent)
        } else {
            Image(systemName: iconName)
                .font(Typography.body())
                .symbolRenderingMode(.monochrome)
                .foregroundStyle(iconColor)
        }
    }

    @ViewBuilder
    private var sensitivityActions: some View {
        ForEach(VADSensitivity.allCases, id: \.self) { sensitivity in
            Button {
                voiceService.setVADSensitivity(sensitivity)
            } label: {
                HStack {
                    Text(sensitivity.title)
                    if voiceService.vadSensitivity == sensitivity {
                        Image(systemName: "checkmark")
                    }
                }
            }
        }
    }

    private func handleTap() {
        if ignoreNextTap {
            ignoreNextTap = false
            return
        }

        switch voiceService.state {
        case .idle:
            Task {
                try? await voiceService.startRecording()
            }

        case .listening:
            voiceService.stopListening()

        case .recording:
            if voiceService.inputMode == .voiceActivityDetection {
                voiceService.cancelCurrentUtterance()
            } else {
                stopAndInsertTranscript()
            }

        case .transcribing:
            if voiceService.inputMode == .voiceActivityDetection {
                voiceService.cancelCurrentUtterance()
            }
        }
    }

    private func handleLongPress() {
        guard voiceService.state == .idle else { return }
        ignoreNextTap = true

        Task {
            try? await voiceService.startListening()
        }
    }

    private func stopAndInsertTranscript() {
        Task {
            let transcript = await voiceService.stopRecording()
            guard !transcript.isEmpty else { return }
            onTranscript(transcript)
        }
    }

    private func updateAnimationState() {
        if voiceService.state == .recording, !reduceMotion {
            isPulsing = false
            withAnimation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true)) {
                isPulsing = true
            }
        } else {
            isPulsing = false
        }

        if voiceService.state == .listening, !reduceMotion {
            isBreathing = false
            withAnimation(.easeInOut(duration: 1.2).repeatForever(autoreverses: true)) {
                isBreathing = true
            }
        } else {
            isBreathing = false
        }
    }
}

private extension VADSensitivity {
    var title: String {
        switch self {
        case .quiet:
            return "Quiet room"
        case .normal:
            return "Normal"
        case .noisy:
            return "Noisy"
        }
    }
}
