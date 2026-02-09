// StepRunner - step/flow execution UI with direction pills and prompt field.

import SwiftUI
import LoopflowCore

struct StepRunner: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.colorScheme) private var colorScheme

    @State private var selectedFlow: String = ""
    @State private var selectedStimulus: Stimulus.Kind = .once
    @State private var cronExpression: String = "0 9 * * *"
    @State private var prompt: String = ""
    @State private var errorMessage: String?
    @State private var showingError = false

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    private var promptPlaceholder: String {
        "Additional context (optional)"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xl) {
            // Configuration: area + direction (wave state)
            VStack(alignment: .leading, spacing: Spacing.md) {
                areaHeader
                directionHeader
            }

            Divider()

            // Execution: flow + stimulus + prompt + run
            VStack(alignment: .leading, spacing: Spacing.xl) {
                flowHeader

                stimulusPicker

                promptField

                runButton
            }
        }
        .padding(Spacing.xl)
        .background(palette.background)
        .alert("Error", isPresented: $showingError) {
            Button("OK") { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "Failed to run step")
        }
        .onAppear {
            selectedFlow = wave.flow
            selectedStimulus = wave.stimulus.kind == .manual ? .once : wave.stimulus.kind
            if let cron = wave.stimulus.cron {
                cronExpression = cron
            }
        }
    }

    // MARK: - Area Header

    private var areaHeader: some View {
        AreaTypeahead(wave: wave) { areas in
            Task {
                try? await repoState.updateWave(wave, area: areas.isEmpty ? nil : areas)
            }
        }
    }

    // MARK: - Direction Header

    private var directionHeader: some View {
        DirectionTypeahead(wave: wave) { directions in
            Task {
                try? await repoState.updateWave(wave, direction: directions.isEmpty ? nil : directions)
            }
        }
    }

    // MARK: - Flow Header

    private var flowHeader: some View {
        FlowTypeahead(wave: wave) { flow in
            selectedFlow = flow
            if !flow.isEmpty {
                Task {
                    try? await repoState.updateWave(wave, flow: flow)
                }
            }
        }
    }

    // MARK: - Stimulus Picker

    private var stimulusPicker: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Stimulus")
                .font(.caption)
                .foregroundStyle(.secondary)

            HStack(spacing: Spacing.xs) {
                stimulusPill(.once, label: "Once", icon: "1.circle")
                stimulusPill(.loop, label: "Loop", icon: "repeat")
                stimulusPill(.watch, label: "Watch", icon: "eye")
                stimulusPill(.cron, label: "Schedule", icon: "clock")
            }

            if selectedStimulus == .cron {
                TextField("0 9 * * *", text: $cronExpression)
                    .textFieldStyle(.plain)
                    .font(.system(.caption, design: .monospaced))
                    .padding(Spacing.sm)
                    .background(palette.surface)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))

                Text("Examples: 0 9 * * * (daily 9am), */30 * * * * (every 30 min)")
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
            }
        }
    }

    private func stimulusPill(_ kind: Stimulus.Kind, label: String, icon: String) -> some View {
        let isSelected = selectedStimulus == kind

        return Button {
            selectedStimulus = kind
        } label: {
            HStack(spacing: Spacing.xs) {
                Image(systemName: icon)
                    .font(.caption2)
                Text(label)
                    .font(.caption)
            }
            .padding(.horizontal, Spacing.md)
            .padding(.vertical, Spacing.sm)
            .background(isSelected ? palette.accent.opacity(0.15) : palette.surface)
            .foregroundStyle(isSelected ? palette.accent : .secondary)
            .clipShape(Capsule())
        }
        .buttonStyle(.plain)
    }

    // MARK: - Prompt Field

    private var promptField: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Additional context (optional)")
                .font(.caption)
                .foregroundStyle(.secondary)

            TextField(promptPlaceholder, text: $prompt, axis: .vertical)
                .textFieldStyle(.plain)
                .padding(Spacing.md)
                .background(palette.surface)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                .lineLimit(3...6)
        }
    }

    // MARK: - Run Button

    private var isWaveActive: Bool {
        wave.status == .running || wave.status == .waiting
    }

    private var runButtonLabel: String {
        switch selectedStimulus {
        case .once, .manual: return selectedFlow.isEmpty ? "Run" : "Run \(selectedFlow)"
        case .loop: return "Start Loop"
        case .watch: return "Start Watching"
        case .cron: return "Start Schedule"
        }
    }

    private var runButtonIcon: String {
        switch selectedStimulus {
        case .once, .manual: return "play.fill"
        case .loop: return "repeat"
        case .watch: return "eye"
        case .cron: return "clock"
        }
    }

    private var runButton: some View {
        Button {
            runFlow()
        } label: {
            HStack(spacing: Spacing.sm) {
                if isWaveActive {
                    ProgressView()
                        .scaleEffect(0.8)
                } else {
                    Image(systemName: runButtonIcon)
                        .font(.title3)
                }
                Text(runButtonLabel)
                    .font(.title3)
                    .fontWeight(.semibold)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, Spacing.lg)
            .background(runButtonDisabled ? Color.gray : palette.accent)
            .foregroundStyle(.white)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        }
        .buttonStyle(.plain)
        .disabled(runButtonDisabled)
        .opacity(runButtonDisabled ? 0.5 : 1)
    }

    private var runButtonDisabled: Bool {
        selectedFlow.isEmpty || isWaveActive
    }

    // MARK: - Actions

    private func runFlow() {
        guard !selectedFlow.isEmpty else { return }

        let stimulus = Stimulus(
            kind: selectedStimulus,
            cron: selectedStimulus == .cron ? cronExpression : nil
        )
        Task {
            do {
                try await repoState.runWave(
                    wave: wave,
                    flow: selectedFlow,
                    stimulus: stimulus
                )
            } catch {
                await MainActor.run {
                    errorMessage = error.localizedDescription
                    showingError = true
                }
            }
        }
    }

}

#Preview {
    let repoState = RepoState()
    repoState.configureMockWaves()
    repoState.flows = [
        Flow(name: "ship", steps: [
            Step(prompt: "implement"),
            Step(prompt: "compress"),
            Step(prompt: "gate")
        ], type: .flow),
        Flow(name: "design-and-ship", steps: [
            Step(prompt: "design"),
            Step(prompt: "implement"),
            Step(prompt: "reduce"),
            Step(prompt: "polish")
        ], type: .flow),
        Flow(name: "design", steps: [Step(prompt: "design")], type: .step),
        Flow(name: "review", steps: [Step(prompt: "review")], type: .step),
        Flow(name: "implement", steps: [Step(prompt: "implement")], type: .step),
        Flow(name: "debug", steps: [Step(prompt: "debug")], type: .step),
    ]
    repoState.availableDirections = ["product-engineer", "designer", "infra-engineer", "ceo"]
    let wave = WaveViewModel(
        api: Wave(
            id: "test",
            name: "test-wave",
            repo: "/tmp/test-repo",
            flow: "ship",
            direction: ["product-engineer"],
            area: ["src/api"]
        ),
        worktreePath: "/tmp/test-worktree"
    )

    return StepRunner(wave: wave)
        .environment(repoState)
        .environment(OutputBuffer())
        .frame(width: 500, height: 600)
}
