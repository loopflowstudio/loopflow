// Landing view shown when no wave is selected.
// Collects a design prompt and launches an interactive `lf design` session.

import SwiftUI
import LoopflowCore

struct StartWaveView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette
    @State private var designPrompt = ""
    @State private var isLaunching = false
    @State private var errorMessage: String?
    @State private var chatState: ChatState?
    @FocusState private var isTextFieldFocused: Bool

    var body: some View {
        Group {
            if let state = chatState {
                WaveChatView(state: state)
            } else {
                VStack(spacing: Spacing.xxl) {
                    Spacer()

                    VStack(spacing: Spacing.lg) {
                        Text("Start designing")
                            .font(Typography.heroTitle())
                            .foregroundStyle(palette.accent)

                        TextField("Describe what you want to build...", text: $designPrompt)
                            .textFieldStyle(.plain)
                            .font(Typography.body())
                            .padding(Spacing.md)
                            .background(palette.surfaceMuted)
                            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                            .frame(maxWidth: 400)
                            .focused($isTextFieldFocused)
                            .onSubmit { startDesigning() }
                            .disabled(isLaunching)

                        Button("Start designing") {
                            startDesigning()
                        }
                        .buttonStyle(DarkButtonStyle())
                        .disabled(isLaunching || trimmedPrompt.isEmpty)

                        if let errorMessage {
                            Text(errorMessage)
                                .font(Typography.caption())
                                .foregroundStyle(Color.statusError)
                        }
                    }

                    Spacer()
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear { isTextFieldFocused = true }
    }

    private var trimmedPrompt: String {
        designPrompt.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func startDesigning() {
        guard !isLaunching else { return }
        guard !trimmedPrompt.isEmpty else { return }
        guard canStartDesignSession() else { return }

        isLaunching = true
        errorMessage = nil

        let prompt = trimmedPrompt
        let state = chatState ?? repoState.chatState(for: "__start_design__")
        chatState = state
        designPrompt = ""

        Task {
            defer { isLaunching = false }
            await state.send(prompt)
        }
    }

    private func canStartDesignSession() -> Bool {
        guard let target = repoState.repoTarget else {
            errorMessage = "Open a repository first."
            return false
        }

        guard let localRepo = target.localURL,
              FileManager.default.fileExists(atPath: localRepo.path()) else {
            errorMessage = "Design launch requires a local repository."
            return false
        }

        let loopflowConfigDirectory = localRepo.appendingPathComponent(".lf", isDirectory: true)
        guard FileManager.default.fileExists(atPath: loopflowConfigDirectory.path()) else {
            errorMessage = "Run `lf init` in this repository before starting a design session."
            return false
        }

        return true
    }
}

#Preview {
    StartWaveView()
        .environment(RepoState())
        .environment(OutputBuffer())
        .frame(width: 600, height: 500)
}
