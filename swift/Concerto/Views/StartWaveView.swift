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
    @FocusState private var isTextFieldFocused: Bool

    private let terminalLauncher = TerminalLauncher()

    var body: some View {
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
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .onAppear { isTextFieldFocused = true }
    }

    private var trimmedPrompt: String {
        designPrompt.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func startDesigning() {
        guard !isLaunching else { return }
        guard !trimmedPrompt.isEmpty else { return }

        guard let repoRoot = repoState.currentRepo else {
            errorMessage = "Open a repository first."
            return
        }

        guard FileManager.default.fileExists(atPath: repoRoot.path()) else {
            errorMessage = "Design launch requires a local repository."
            return
        }

        isLaunching = true
        errorMessage = nil

        Task {
            defer { isLaunching = false }

            do {
                let escapedPrompt = terminalLauncher.escapeShellSingleQuotes(trimmedPrompt)
                let command = "lf design -c '\(escapedPrompt)'"
                try terminalLauncher.launchTerminal(.warp, at: repoRoot, command: command)
            } catch {
                errorMessage = error.localizedDescription
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
