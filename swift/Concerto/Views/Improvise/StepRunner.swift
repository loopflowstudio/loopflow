// StepRunner - step/flow execution UI with direction pills and prompt field.

import SwiftUI
import LoopflowCore

struct StepRunner: View {
    let wave: Wave

    @Environment(RepoState.self) private var repoState
    @Environment(SessionState.self) private var sessionState
    @Environment(\.colorScheme) private var colorScheme

    @State private var selectedStep: String = "design"
    @State private var prompt: String = ""
    @State private var isRunning = false
    @State private var showingAllSteps = false
    @State private var errorMessage: String?
    @State private var showingError = false

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    // Common steps shown prominently with descriptions for tooltips
    private let commonSteps: [(name: String, description: String)] = [
        ("review", "Analyze architecture, complexity, quality"),
        ("design", "Interactive session to plan changes"),
        ("implement", "Build from a design doc"),
        ("debug", "Fix an error (paste it in the prompt)")
    ]

    // 4-column grid for step buttons
    private let gridColumns = Array(repeating: GridItem(.flexible()), count: 4)

    // Step names for comparisons
    private var commonStepNames: [String] {
        commonSteps.map(\.name)
    }

    // All available steps from repo
    private var allSteps: [Flow] {
        repoState.flows.filter { $0.type == .step }
    }

    // Dynamic prompt placeholder based on selected step
    private var promptPlaceholder: String {
        switch selectedStep {
        case "debug":
            return "Paste the error message or describe the bug"
        case "review":
            return "What aspect to focus on? (e.g., security, performance)"
        case "design":
            return "What are you trying to build or change?"
        case "implement":
            return "Any specific requirements or constraints?"
        default:
            return "e.g., focus on auth endpoints"
        }
    }

    // All available flows from repo
    private var allFlows: [Flow] {
        repoState.flows.filter { $0.type == .flow }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xl) {
            // Header with area and change button
            areaHeader

            // Direction pills
            DirectionPills(wave: wave)

            // Step grid
            stepGrid

            // Flow dropdown (secondary)
            if !allFlows.isEmpty {
                flowSelector
            }

            // Prompt field
            promptField

            // Run button
            runButton
        }
        .padding(Spacing.xl)
        .background(palette.background)
        .alert("Error", isPresented: $showingError) {
            Button("OK") { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "Failed to run step")
        }
        .onAppear {
            initializeSelection()
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

    // MARK: - Step Grid

    private var stepGrid: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Quick Steps")
                .font(.caption)
                .foregroundStyle(.secondary)

            LazyVGrid(columns: gridColumns, spacing: Spacing.sm) {
                ForEach(commonSteps, id: \.name) { step in
                    stepButton(step.name, description: step.description)
                }

                if !showingAllSteps && allSteps.count > commonStepNames.count {
                    Button {
                        showingAllSteps = true
                    } label: {
                        Text("More...")
                            .font(.subheadline)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, Spacing.md)
                            .background(palette.surface)
                            .foregroundStyle(.secondary)
                            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                    }
                    .buttonStyle(.plain)
                }
            }

            // Show additional steps when expanded
            if showingAllSteps {
                let additionalSteps = allSteps.filter { !commonStepNames.contains($0.name) }
                if !additionalSteps.isEmpty {
                    LazyVGrid(columns: gridColumns, spacing: Spacing.sm) {
                        ForEach(additionalSteps) { step in
                            stepButton(step.name, description: nil)
                        }
                    }
                    .padding(.top, Spacing.xs)
                }
            }
        }
    }

    private func stepButton(_ step: String, description: String?) -> some View {
        let isSelected = selectedStep == step

        return Button {
            selectedStep = step
        } label: {
            Text(step)
                .font(.subheadline)
                .fontWeight(isSelected ? .semibold : .regular)
                .frame(maxWidth: .infinity)
                .padding(.vertical, Spacing.md)
                .background(isSelected ? palette.accent : palette.surface)
                .foregroundStyle(isSelected ? .white : .primary)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        }
        .buttonStyle(.plain)
        .help(description ?? step)
    }

    // MARK: - Flow Selector

    private var flowSelector: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Or run a flow")
                .font(.caption)
                .foregroundStyle(.secondary)

            HStack {
                Picker("Flow", selection: $selectedStep) {
                    ForEach(allFlows) { flow in
                        Label(flow.name, systemImage: "arrow.triangle.branch")
                            .tag(flow.name)
                    }
                }
                .pickerStyle(.menu)
                .frame(maxWidth: 200)

                if let flow = allFlows.first(where: { $0.name == selectedStep }) {
                    let stepNames = flow.steps.map(\.prompt).joined(separator: " \u{2192} ")
                    Text(stepNames)
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
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
            runStep()
        } label: {
            HStack(spacing: Spacing.sm) {
                if isRunning {
                    ProgressView()
                        .scaleEffect(0.8)
                } else {
                    Image(systemName: "play.fill")
                        .font(.title3)
                }
                Text("Run \(selectedStep)")
                    .font(.title3)
                    .fontWeight(.semibold)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, Spacing.lg)
            .background(Color.green)
            .foregroundStyle(.white)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        }
        .buttonStyle(.plain)
        .disabled(isRunning || wave.worktreePath == nil)
        .opacity(wave.worktreePath == nil ? 0.5 : 1)
    }

    // MARK: - Actions

    private func initializeSelection() {
        // Default to wave's configured flow/step, or "design"
        if !wave.flow.isEmpty {
            selectedStep = wave.flow
        } else if allSteps.contains(where: { $0.name == "design" }) {
            selectedStep = "design"
        } else if let first = allSteps.first {
            selectedStep = first.name
        }
    }

    private func runStep() {
        guard let path = wave.worktreePath else { return }

        // Check if this is an interactive step
        let isInteractive = allSteps.first(where: { $0.name == selectedStep }) != nil

        if isInteractive {
            // Launch interactive session with prompt if provided
            let promptArg = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
            sessionState.launchInteractiveSession(
                waveId: wave.id,
                step: selectedStep,
                worktreePath: path,
                prompt: promptArg.isEmpty ? nil : promptArg
            )
        } else {
            // Run as auto step via daemon
            isRunning = true
            Task {
                do {
                    try await repoState.runWave(
                        wave: wave,
                        flow: selectedStep,
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

}

#Preview {
    let repoState = RepoState()
    repoState.configureMockWaves()
    repoState.flows = [
        Flow(name: "design", steps: [Step(prompt: "design")], type: .step),
        Flow(name: "review", steps: [Step(prompt: "review")], type: .step),
        Flow(name: "implement", steps: [Step(prompt: "implement")], type: .step),
        Flow(name: "debug", steps: [Step(prompt: "debug")], type: .step),
        Flow(name: "ship", steps: [
            Step(prompt: "implement"),
            Step(prompt: "compress"),
            Step(prompt: "gate")
        ], type: .flow)
    ]
    repoState.directions = [
        Direction(id: "product-engineer", name: "product-engineer", content: "", path: URL(fileURLWithPath: "/"))
    ]

    let wave = Wave(
        id: "test",
        name: "test-wave",
        area: ["src/api"],
        direction: ["product-engineer"],
        flow: "design",
        repo: "/tmp/test-repo",
        worktreePath: "/tmp/test-worktree"
    )

    return StepRunner(wave: wave)
        .environment(repoState)
        .environment(SessionState())
        .frame(width: 500, height: 600)
}
