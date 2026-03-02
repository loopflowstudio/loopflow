// StepRunner - wave execution UI with flow selection and run/auto controls.

import SwiftUI
import LoopflowCore

struct StepRunner: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var selectedFlow: String = ""
    @State private var selectedAgent: String = ""
    @State private var triggerSignal: Trigger.Signal = .repo
    @State private var prompt: String = ""
    @State private var isSendingRun = false
    @State private var isSendingTrigger = false
    @State private var errorMessage: String?
    @State private var showingError = false

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xl) {
            VStack(alignment: .leading, spacing: Spacing.md) {
                areaHeader
                directionHeader
                agentHeader
            }

            Divider()

            VStack(alignment: .leading, spacing: Spacing.xl) {
                flowHeader
                promptField

                if wave.hasActiveTrigger {
                    activeTriggerView
                } else {
                    HStack(spacing: Spacing.md) {
                        runButton
                        addTriggerButton
                    }
                }
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
            selectedAgent = wave.agent ?? ""
            if let t = wave.trigger {
                triggerSignal = t.signal
            }
        }
    }

    // MARK: - Configuration

    private var areaHeader: some View {
        AreaTypeahead(wave: wave) { areas in
            Task {
                try? await repoState.updateWave(wave, area: areas.isEmpty ? nil : areas)
            }
        }
    }

    private var directionHeader: some View {
        DirectionTypeahead(wave: wave) { directions in
            Task {
                try? await repoState.updateWave(wave, direction: directions.isEmpty ? nil : directions)
            }
        }
    }

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

    @ViewBuilder
    private var agentHeader: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text("Model")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            if repoState.supportedHarnesses.isEmpty {
                TextField("Default (claude:opus)", text: $selectedAgent)
                    .textFieldStyle(.plain)
                    .font(Typography.code(12))
                    .padding(Spacing.sm)
                    .background(palette.surface)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                    .onSubmit {
                        persistAgent(selectedAgent)
                    }
            } else {
                Picker("Model", selection: $selectedAgent) {
                    Text("Default").tag("")
                    ForEach(agentPickerOptions, id: \.self) { agent in
                        Text(agent).tag(agent)
                    }
                }
                .labelsHidden()
                .onChange(of: selectedAgent) { _, newValue in
                    persistAgent(newValue)
                }
            }
        }
    }

    private var agentPickerOptions: [String] {
        var options = repoState.supportedHarnesses
        if let activeAgent = wave.agent, !activeAgent.isEmpty, !options.contains(activeAgent) {
            options.append(activeAgent)
        }
        return options.sorted()
    }

    // MARK: - Fields

    private var promptField: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Additional context (optional)")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            TextField("e.g. focus on error handling", text: $prompt, axis: .vertical)
                .textFieldStyle(.plain)
                .padding(Spacing.md)
                .background(palette.surface)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                .lineLimit(3...6)
        }
    }

    // MARK: - Active Trigger

    private var activeTriggerView: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            if let t = wave.trigger {
                HStack {
                    Image(systemName: t.icon)
                    Text(t.label)
                        .fontWeight(.semibold)
                    if let flow = t.flow {
                        Text(flow)
                            .font(Typography.code(11))
                            .foregroundStyle(palette.textSecondary)
                    }
                    Spacer()
                    Button {
                        removeActiveTrigger(t.id)
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(palette.textSecondary)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Remove trigger")
                }
                .padding(Spacing.md)
                .background(palette.surface)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
            }
        }
    }

    // MARK: - Action Buttons

    private var actionsDisabled: Bool {
        selectedFlow.isEmpty || wave.status == .running || wave.status == .waiting
    }

    private var runDisabled: Bool {
        isSendingRun || actionsDisabled
    }

    private var triggerDisabled: Bool {
        isSendingTrigger || actionsDisabled
    }

    private var runButton: some View {
        Button {
            runOnce()
        } label: {
            HStack(spacing: Spacing.sm) {
                if isSendingRun {
                    ProgressView()
                        .scaleEffect(0.8)
                } else {
                    Image(systemName: "play.fill")
                }
                Text("Run")
                    .fontWeight(.semibold)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, Spacing.lg)
            .background(runDisabled ? Color.statusNeutral : palette.accent)
            .foregroundStyle(.white)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        }
        .buttonStyle(.plain)
        .disabled(runDisabled)
        .opacity(runDisabled ? 0.5 : 1)
    }

    private var addTriggerButton: some View {
        HStack(spacing: 0) {
            Button {
                addTrigger()
            } label: {
                HStack(spacing: Spacing.sm) {
                    if isSendingTrigger {
                        ProgressView()
                            .scaleEffect(0.8)
                    } else {
                        Image(systemName: triggerSignal.icon)
                    }
                    Text(triggerSignal.label)
                        .fontWeight(.semibold)
                }
                .padding(.vertical, Spacing.lg)
                .padding(.leading, Spacing.lg)
                .padding(.trailing, Spacing.sm)
            }
            .buttonStyle(.plain)

            Rectangle()
                .fill(palette.border)
                .frame(width: 1, height: 20)

            Menu {
                ForEach(Trigger.Signal.allCases, id: \.rawValue) { signal in
                    Button { triggerSignal = signal } label: {
                        Label(signal.label, systemImage: signal.icon)
                    }
                }
            } label: {
                Image(systemName: "chevron.down")
                    .font(Typography.caption(10))
                    .padding(.vertical, Spacing.lg)
                    .padding(.horizontal, Spacing.sm)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Change trigger signal")
        }
        .background(triggerDisabled ? Color.statusNeutral : palette.surface)
        .foregroundStyle(triggerDisabled ? .white : palette.text)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        .disabled(triggerDisabled)
        .opacity(triggerDisabled ? 0.5 : 1)
    }

    // MARK: - Actions

    private func runOnce() {
        guard !selectedFlow.isEmpty else { return }

        isSendingRun = true
        Task {
            do {
                try await repoState.runWave(
                    wave: wave,
                    flow: selectedFlow
                )
            } catch {
                showError(error)
            }
            await MainActor.run {
                isSendingRun = false
            }
        }
    }

    private func addTrigger() {
        guard !selectedFlow.isEmpty else { return }

        isSendingTrigger = true
        Task {
            do {
                try await repoState.addTrigger(
                    wave: wave,
                    signal: triggerSignal
                )
            } catch {
                showError(error)
            }
            await MainActor.run {
                isSendingTrigger = false
            }
        }
    }

    private func removeActiveTrigger(_ triggerId: String) {
        Task {
            do {
                try await repoState.removeTrigger(wave: wave, triggerId: triggerId)
            } catch {
                showError(error)
            }
        }
    }

    private func persistAgent(_ agent: String) {
        let trimmed = agent.trimmingCharacters(in: .whitespacesAndNewlines)
        Task {
            do {
                try await repoState.updateWave(wave, agent: trimmed)
            } catch {
                showError(error)
            }
        }
    }

    @MainActor
    private func showError(_ error: Error) {
        errorMessage = error.localizedDescription
        showingError = true
    }
}

#Preview {
    let repoState = RepoState()
    repoState.configureMockWaves()
    repoState.flows = [
        Flow(name: "build", steps: [
            Step(prompt: "implement"),
            Step(prompt: "compress"),
            Step(prompt: "gate"),
            Step(prompt: "update-wave")
        ], type: .flow),
        Flow(name: "ship", steps: [
            Step(prompt: "design"),
            Step(prompt: "build"),
            Step(prompt: "review")
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
    repoState.availableDirections = ["ux", "clarity", "infra", "ceo"]
    let wave = WaveViewModel(
        api: Wave(
            id: "test",
            name: "test-wave",
            repo: "/tmp/test-repo",
            flow: "build",
            direction: ["clarity"],
            area: ["src/api"]
        ),
        worktreePath: "/tmp/test-worktree"
    )

    return StepRunner(wave: wave)
        .environment(repoState)
        .frame(width: 500, height: 600)
}
