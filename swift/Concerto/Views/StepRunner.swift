// StepRunner - step/flow execution UI with direction pills and prompt field.

import SwiftUI
import LoopflowCore

struct StepRunner: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer
    @Environment(\.colorScheme) private var colorScheme

    @State private var selectedFlow: String = ""
    @State private var prompt: String = ""
    @State private var isRunning = false
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

            // Execution: flow + prompt + run
            VStack(alignment: .leading, spacing: Spacing.xl) {
                flowHeader

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
            selectedFlow = wave.flow.isEmpty ? "ship" : wave.flow
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

    private var runButton: some View {
        Button {
            runFlow()
        } label: {
            HStack(spacing: Spacing.sm) {
                if isRunning {
                    ProgressView()
                        .scaleEffect(0.8)
                } else {
                    Image(systemName: "play.fill")
                        .font(.title3)
                }
                Text("Run \(selectedFlow)")
                    .font(.title3)
                    .fontWeight(.semibold)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, Spacing.lg)
            .background(selectedFlow.isEmpty ? Color.gray : palette.accent)
            .foregroundStyle(.white)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        }
        .buttonStyle(.plain)
        .disabled(isRunning || selectedFlow.isEmpty)
        .opacity(selectedFlow.isEmpty ? 0.5 : 1)
    }

    // MARK: - Actions

    private func runFlow() {
        guard !selectedFlow.isEmpty else { return }

        isRunning = true
        Task {
            do {
                try await repoState.runWave(
                    wave: wave,
                    flow: selectedFlow,
                    stimulus: Stimulus(kind: .once)
                )
            } catch {
                await MainActor.run {
                    errorMessage = error.localizedDescription
                    showingError = true
                }
            }
            await MainActor.run {
                isRunning = false
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
