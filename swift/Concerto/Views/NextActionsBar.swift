// NextActionsBar - sticky footer for next actions after running steps.

import SwiftUI
import LoopflowCore

struct NextActionsBar: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(\.colorScheme) private var colorScheme

    @State private var showingStimulusPicker = false
    @State private var isArchiving = false
    @State private var errorMessage: String?
    @State private var showingError = false

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    private var stepsRunCount: Int {
        wave.recentSteps.count
    }

    var body: some View {
        HStack(spacing: Spacing.lg) {
            // Status message
            HStack(spacing: Spacing.sm) {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundStyle(.green)

                Text("\(stepsRunCount) step\(stepsRunCount == 1 ? "" : "s") run")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            Spacer()

            // Set Stimulus button (primary action)
            Button {
                showingStimulusPicker = true
            } label: {
                HStack(spacing: Spacing.xs) {
                    Image(systemName: "repeat")
                        .font(.caption)
                    Text("Set Stimulus")
                }
            }
            .buttonStyle(.borderedProminent)
            .sheet(isPresented: $showingStimulusPicker) {
                StimulusPicker(wave: wave, isPresented: $showingStimulusPicker)
            }

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
                            .font(.caption)
                    }
                    Text("Archive")
                }
            }
            .buttonStyle(DarkButtonStyle())
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
        // Archive by stopping and setting stimulus to manual (paused)
        isArchiving = true
        Task {
            do {
                try await repoState.updateWave(wave, stimulus: Stimulus(kind: .manual), status: .paused)
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

// MARK: - Stimulus Picker Sheet

struct StimulusPicker: View {
    let wave: WaveViewModel
    @Binding var isPresented: Bool

    @Environment(RepoState.self) private var repoState
    @Environment(\.colorScheme) private var colorScheme

    @State private var selectedKind: Stimulus.Kind = .loop
    @State private var cronExpression: String = "0 9 * * *"
    @State private var isSaving = false

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    var body: some View {
        VStack(spacing: Spacing.xl) {
            // Header
            HStack {
                Text("Set Stimulus")
                    .font(.title2)
                    .fontWeight(.semibold)
                Spacer()
                Button("Cancel") {
                    isPresented = false
                }
                .buttonStyle(.plain)
            }

            // Options
            VStack(spacing: Spacing.md) {
                stimulusOption(
                    kind: .loop,
                    title: "Loop",
                    description: "Run continuously until stopped",
                    icon: "repeat"
                )

                stimulusOption(
                    kind: .once,
                    title: "Once",
                    description: "Run one time then stop",
                    icon: "1.circle"
                )

                stimulusOption(
                    kind: .watch,
                    title: "Watch",
                    description: "Run when files change on main",
                    icon: "eye"
                )

                stimulusOption(
                    kind: .cron,
                    title: "Schedule",
                    description: "Run on a schedule",
                    icon: "clock"
                )
            }

            // Cron expression input
            if selectedKind == .cron {
                VStack(alignment: .leading, spacing: Spacing.sm) {
                    Text("Cron expression")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    TextField("0 9 * * *", text: $cronExpression)
                        .textFieldStyle(.roundedBorder)

                    Text("Examples: 0 9 * * * (daily 9am), */30 * * * * (every 30 min)")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }
            }

            Spacer()

            // Save button
            Button {
                saveStimulus()
            } label: {
                HStack {
                    if isSaving {
                        ProgressView()
                            .scaleEffect(0.8)
                    }
                    Text("Start Running")
                        .fontWeight(.semibold)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, Spacing.md)
            }
            .buttonStyle(.borderedProminent)
            .disabled(isSaving)
        }
        .padding(Spacing.xl)
        .frame(width: 400, height: 500)
    }

    private func stimulusOption(kind: Stimulus.Kind, title: String, description: String, icon: String) -> some View {
        let isSelected = selectedKind == kind

        return Button {
            selectedKind = kind
        } label: {
            HStack(spacing: Spacing.md) {
                Image(systemName: icon)
                    .font(.title3)
                    .foregroundStyle(isSelected ? palette.accent : .secondary)
                    .frame(width: 32)

                VStack(alignment: .leading, spacing: Spacing.xxs) {
                    Text(title)
                        .font(.subheadline)
                        .fontWeight(isSelected ? .semibold : .regular)

                    Text(description)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                if isSelected {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(palette.accent)
                }
            }
            .padding(Spacing.md)
            .background(isSelected ? palette.accent.opacity(0.1) : palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
        .buttonStyle(.plain)
    }

    private func saveStimulus() {
        isSaving = true
        Task {
            let stimulus = Stimulus(
                kind: selectedKind,
                cron: selectedKind == .cron ? cronExpression : nil
            )

            do {
                // Set stimulus and unpause
                try await repoState.updateWave(wave, stimulus: stimulus, status: .idle)
                // Start running
                try await repoState.runWave(wave: wave, stimulus: stimulus)
                await MainActor.run {
                    isPresented = false
                }
            } catch {
                // Handle error silently - UI will show previous state
            }
            await MainActor.run {
                isSaving = false
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
            direction: ["product-engineer"],
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
