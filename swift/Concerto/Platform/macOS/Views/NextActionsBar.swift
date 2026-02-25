// NextActionsBar - sticky footer for next actions after running steps.

import SwiftUI
import LoopflowCore

struct NextActionsBar: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var isArchiving = false
    @State private var errorMessage: String?
    @State private var showingError = false

    private var stepsRunCount: Int {
        wave.recentSteps.count
    }

    var body: some View {
        HStack(spacing: Spacing.lg) {
            // Status message
            HStack(spacing: Spacing.sm) {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(Color.statusSuccess)

                Text("\(stepsRunCount) step\(stepsRunCount == 1 ? "" : "s") run")
                    .font(Typography.body())
                    .foregroundStyle(palette.textSecondary)
            }

            Spacer()

            // Archive button
            Button {
                archive()
            } label: {
                HStack(spacing: Spacing.xs) {
                    if isArchiving {
                        ProgressView()
                            .scaleEffect(0.6)
                    } else {
                        Image(systemName: "archivebox")
                            .font(Typography.caption())
                    }
                    Text("Archive")
                }
            }
            .buttonStyle(DestructiveButtonStyle())
            .disabled(isArchiving)
        }
        .padding(.horizontal, Spacing.xl)
        .padding(.vertical, Spacing.md)
        .background(palette.surface)
        .alert("Error", isPresented: $showingError) {
            Button("OK") { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "An error occurred")
        }
    }

    private func archive() {
        isArchiving = true
        Task {
            do {
                try await repoState.updateWave(wave, status: .paused)
            } catch {
                await MainActor.run {
                    errorMessage = error.localizedDescription
                    showingError = true
                }
            }
            await MainActor.run {
                isArchiving = false
            }
        }
    }
}

#Preview {
    let repoState = RepoState()
    repoState.configureMockWaves()

    let wave = WaveViewModel(
        api: Wave(
            id: "test",
            name: "test-wave",
            repo: "/tmp/test-repo",
            flow: "design",
            direction: ["clarity"],
            area: ["src/api"]
        ),
        worktreePath: "/tmp/test-worktree",
        hasDiff: true,
        recentSteps: [
            StepRun(id: "1", step: "design", repo: "", worktree: "", status: "completed", startedAt: Date(), endedAt: nil, model: "", runMode: ""),
            StepRun(id: "2", step: "implement", repo: "", worktree: "", status: "completed", startedAt: Date(), endedAt: nil, model: "", runMode: "")
        ]
    )

    return VStack {
        Spacer()
        NextActionsBar(wave: wave)
    }
    .environment(repoState)
    .frame(width: 600, height: 200)
}

