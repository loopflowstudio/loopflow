// Landing view shown when no wave is selected.
// Single text field to start a new wave by name.

import SwiftUI
import LoopflowCore

struct StartWaveView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.palette) private var palette
    @State private var waveName = ""
    @State private var isCreating = false
    @State private var errorMessage: String?
    @FocusState private var isTextFieldFocused: Bool

    var body: some View {
        VStack(spacing: Spacing.xxl) {
            Spacer()

            VStack(spacing: Spacing.lg) {
                Text("Start a wave")
                    .font(Typography.heroTitle())
                    .foregroundStyle(palette.accent)

                TextField("What do you want to build?", text: $waveName)
                    .textFieldStyle(.plain)
                    .font(Typography.body())
                    .padding(Spacing.md)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    .frame(maxWidth: 400)
                    .focused($isTextFieldFocused)
                    .onSubmit { createWave() }
                    .disabled(isCreating)

                if let errorMessage {
                    Text(errorMessage)
                        .font(Typography.caption())
                        .foregroundStyle(Color.statusError)
                }
            }

            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear { isTextFieldFocused = true }
    }

    private func createWave() {
        guard !isCreating else { return }
        isCreating = true
        errorMessage = nil

        Task {
            defer { isCreating = false }

            do {
                try await repoState.ensureReadyToCreateWave(outputBuffer: outputBuffer)
                let name = waveName.trimmingCharacters(in: .whitespacesAndNewlines)
                try await repoState.createWave(name: name)
                NotificationCenter.default.post(name: .editWaveName, object: nil)
            } catch {
                errorMessage = repoState.isConnected
                    ? error.localizedDescription
                    : repoState.connectionSummary
            }
        }
    }
}

#Preview {
    StartWaveView()
        .environment(RepoState())
        .environment(OutputBuffer())
        .frame(width: 600, height: 500)
}
