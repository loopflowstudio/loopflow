// Landing view shown when no wave is selected.
// Collects a wave name and kicks off the default design-first flow.

import SwiftUI
import LoopflowCore

struct CatchWaveView: View {
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
                Text("Ride a wave")
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
                    .onSubmit { catchWave() }
                    .disabled(isLaunching)

                Button("Ride wave") {
                    catchWave()
                }
                .buttonStyle(DarkButtonStyle())
                .disabled(isLaunching)

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

    private func catchWave() {
        guard !isLaunching else { return }
        guard canCatchWave() else { return }

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

    private func canCatchWave() -> Bool {
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
            errorMessage = "Run `lf init` in this repository before riding a wave."
            return false
        }

        return true
    }
}

#Preview {
    CatchWaveView()
        .environment(RepoState())
        .environment(OutputBuffer())
        .frame(width: 600, height: 500)
}
