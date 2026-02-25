// Landing view shown when no wave is selected.
// Collects a wave name and starts the default design-first flow.

import SwiftUI
import LoopflowCore

struct StartWaveView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette
    @State private var waveName = ""
    @State private var isLaunching = false
    @State private var errorMessage: String?
    @FocusState private var isTextFieldFocused: Bool

    var body: some View {
        VStack(spacing: Spacing.xxl) {
            Spacer()

            VStack(spacing: Spacing.lg) {
                Text("Start a wave")
                    .font(Typography.heroTitle())
                    .foregroundStyle(palette.accent)

                TextField("Wave name", text: $waveName)
                    .textFieldStyle(.plain)
                    .font(Typography.body())
                    .padding(Spacing.md)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    .frame(maxWidth: 400)
                    .focused($isTextFieldFocused)
                    .onSubmit { startWave() }
                    .disabled(isLaunching)

                Button("Start wave") {
                    startWave()
                }
                .buttonStyle(DarkButtonStyle())
                .disabled(isLaunching || trimmedName.isEmpty)

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

    private var trimmedName: String {
        waveName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func startWave() {
        guard !isLaunching else { return }
        guard !trimmedName.isEmpty else { return }
        guard canStartWave() else { return }

        isLaunching = true
        errorMessage = nil
        let name = trimmedName

        Task {
            defer { isLaunching = false }
            do {
                _ = try await repoState.createAndRunWave(name: name)
                waveName = ""
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    private func canStartWave() -> Bool {
        guard let target = repoState.repoTarget else {
            errorMessage = "Open a repository first."
            return false
        }

        guard let localRepo = target.localURL,
              FileManager.default.fileExists(atPath: localRepo.path()) else {
            errorMessage = "Wave launch requires a local repository."
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

