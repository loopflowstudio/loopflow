// StepRunner - wave execution UI with flow selection and run/auto controls.

import SwiftUI
import LoopflowCore

struct StepRunner: View {
    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    @State private var selectedFlow: String = ""
    @State private var autoMode: Stimulus.Kind = .loop
    @State private var cronExpression: String = "0 9 * * *"
    @State private var prompt: String = ""
    @State private var isSendingRun = false
    @State private var isSendingAuto = false
    @State private var errorMessage: String?
    @State private var showingError = false

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xl) {
            VStack(alignment: .leading, spacing: Spacing.md) {
                areaHeader
                directionHeader
            }

            Divider()

            VStack(alignment: .leading, spacing: Spacing.xl) {
                flowHeader
                promptField

                if wave.hasActiveStimulus {
                    activeStimulusView
                } else {
                    if autoMode == .cron {
                        cronField
                    }

                    HStack(spacing: Spacing.md) {
                        runButton
                        autoButton
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
            if let s = wave.stimulus {
                autoMode = s.kind
                if let cron = s.cron {
                    cronExpression = cron
                }
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

    // MARK: - Fields

    private var promptField: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Additional context (optional)")
                .font(Typography.caption())
                .foregroundStyle(.secondary)

            TextField("e.g. focus on error handling", text: $prompt, axis: .vertical)
                .textFieldStyle(.plain)
                .padding(Spacing.md)
                .background(palette.surface)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
                .lineLimit(3...6)
        }
    }

    private var cronField: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            TextField("0 9 * * *", text: $cronExpression)
                .textFieldStyle(.plain)
                .font(Typography.code(11))
                .padding(Spacing.sm)
                .background(palette.surface)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                .accessibilityLabel("Cron expression")

            Text("Examples: 0 9 * * * (daily 9am), */30 * * * * (every 30 min)")
                .font(Typography.caption(10))
                .foregroundStyle(.tertiary)
        }
    }

    // MARK: - Active Stimulus

    private var activeStimulusView: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            if let s = wave.stimulus {
                HStack {
                    Image(systemName: s.icon)
                    Text(s.label)
                        .fontWeight(.semibold)
                    if let cron = s.cron {
                        Text(cron)
                            .font(Typography.code(11))
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button {
                        removeActiveStimulus(s.id)
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(.secondary)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Remove stimulus")
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

    private var autoDisabled: Bool {
        isSendingAuto || actionsDisabled
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

    private var autoButton: some View {
        HStack(spacing: 0) {
            Button {
                addAutoStimulus()
            } label: {
                HStack(spacing: Spacing.sm) {
                    if isSendingAuto {
                        ProgressView()
                            .scaleEffect(0.8)
                    } else {
                        Image(systemName: autoMode.icon)
                    }
                    Text(autoMode.label)
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
                ForEach(Stimulus.Kind.allCases, id: \.rawValue) { kind in
                    Button { autoMode = kind } label: {
                        Label(kind.label, systemImage: kind.icon)
                    }
                }
            } label: {
                Image(systemName: "chevron.down")
                    .font(Typography.caption(10))
                    .padding(.vertical, Spacing.lg)
                    .padding(.horizontal, Spacing.sm)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Change auto mode")
        }
        .background(autoDisabled ? Color.statusNeutral : palette.surface)
        .foregroundStyle(autoDisabled ? .white : palette.text)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        .disabled(autoDisabled)
        .opacity(autoDisabled ? 0.5 : 1)
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

    private func addAutoStimulus() {
        guard !selectedFlow.isEmpty else { return }

        isSendingAuto = true
        let cron = autoMode == .cron ? cronExpression : nil
        Task {
            do {
                try await repoState.addStimulus(
                    wave: wave,
                    kind: autoMode,
                    cron: cron
                )
            } catch {
                showError(error)
            }
            await MainActor.run {
                isSendingAuto = false
            }
        }
    }

    private func removeActiveStimulus(_ stimulusId: String) {
        Task {
            do {
                try await repoState.removeStimulus(wave: wave, stimulusId: stimulusId)
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
        .frame(width: 500, height: 600)
}
