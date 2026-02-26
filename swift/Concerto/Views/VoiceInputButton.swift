import SwiftUI
import LoopflowCore

struct VoiceInputButton: View {
    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @Bindable var voiceService: VoiceInputService
    let onTranscript: (String) -> Void

    @State private var holdTask: Task<Void, Never>?
    @State private var isPressing = false
    @State private var isHoldRecording = false
    @State private var isPulsing = false

    private var buttonSize: CGFloat { platformVoiceButtonSize }

    private var iconName: String {
        switch voiceService.state {
        case .idle:
            return "mic"
        case .recording:
            return "mic.fill"
        case .transcribing:
            return "waveform"
        }
    }

    private var iconColor: Color {
        switch voiceService.state {
        case .idle:
            return palette.textSecondary
        case .recording:
            return palette.accent
        case .transcribing:
            return .statusInfo
        }
    }

    private var backgroundColor: Color {
        switch voiceService.state {
        case .idle:
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
            return "Tap to start recording, or press and hold to record while held."
        case .recording:
            return "Tap to stop recording."
        case .transcribing:
            return "Transcribing your audio."
        }
    }

    var body: some View {
        ZStack {
            Circle()
                .fill(backgroundColor)

            Image(systemName: iconName)
                .font(Typography.body())
                .foregroundStyle(iconColor)
        }
        .frame(width: buttonSize, height: buttonSize)
        .scaleEffect(voiceService.state == .recording && !reduceMotion ? (isPulsing ? 1.08 : 1) : 1)
        .animation(DesignAnimation.standard(reduceMotion), value: voiceService.state)
        .animation(DesignAnimation.standard(reduceMotion), value: isPulsing)
        .contentShape(Circle())
        .gesture(pressGesture)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Voice input")
        .accessibilityHint(accessibilityHint)
        .accessibilityAddTraits(.isButton)
        .onChange(of: voiceService.state, initial: true) { _, _ in
            updatePulseAnimation()
        }
    }

    private var pressGesture: some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { _ in
                guard !isPressing else { return }
                isPressing = true
                scheduleHoldRecording()
            }
            .onEnded { _ in
                isPressing = false
                let wasHoldRecording = isHoldRecording
                holdTask?.cancel()
                holdTask = nil

                if wasHoldRecording {
                    isHoldRecording = false
                    stopAndInsertTranscript()
                } else {
                    toggleTapRecording()
                }
            }
    }

    private func scheduleHoldRecording() {
        guard voiceService.state == .idle else { return }

        holdTask?.cancel()
        holdTask = Task {
            try? await Task.sleep(for: .milliseconds(220))
            guard !Task.isCancelled else { return }
            guard isPressing else { return }

            isHoldRecording = true
            do {
                try await voiceService.startRecording()
            } catch {
                isHoldRecording = false
            }
        }
    }

    private func toggleTapRecording() {
        switch voiceService.state {
        case .idle:
            Task {
                try? await voiceService.startRecording()
            }
        case .recording:
            stopAndInsertTranscript()
        case .transcribing:
            break
        }
    }

    private func stopAndInsertTranscript() {
        Task {
            let transcript = await voiceService.stopRecording()
            guard !transcript.isEmpty else { return }
            onTranscript(transcript)
        }
    }

    private func updatePulseAnimation() {
        guard voiceService.state == .recording, !reduceMotion else {
            isPulsing = false
            return
        }

        isPulsing = false
        withAnimation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true)) {
            isPulsing = true
        }
    }
}
