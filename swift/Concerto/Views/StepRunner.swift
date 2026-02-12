// StepRunner - wave execution UI with flow selection and run/loop controls.

import SwiftUI
import LoopflowCore

struct StepRunner: View {
    private enum AutoMode: String, CaseIterable {
        case loop
        case watch
        case cron

        init(stimulus kind: Stimulus.Kind) {
            switch kind {
            case .loop:
                self = .loop
            case .watch:
                self = .watch
            case .cron:
                self = .cron
            case .once, .manual:
                self = .loop
            }
        }

        var stimulusKind: Stimulus.Kind {
            switch self {
            case .loop:
                return .loop
            case .watch:
                return .watch
            case .cron:
                return .cron
            }
        }

        var label: String {
            self == .cron ? "Schedule" : rawValue.capitalized
        }

        var icon: String {
            switch self {
            case .loop:
                return "repeat"
            case .watch:
                return "eye"
            case .cron:
                return "clock"
            }
        }
    }

    let wave: WaveViewModel

    @Environment(RepoState.self) private var repoState
    @Environment(\.colorScheme) private var colorScheme

    @State private var selectedFlow: String = ""
    @State private var autoMode: AutoMode = .loop
    @State private var cronExpression: String = "0 9 * * *"
    @State private var prompt: String = ""
    @State private var isSendingRun = false
    @State private var errorMessage: String?
    @State private var showingError = false

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

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

                if autoMode == .cron {
                    cronField
                }

                HStack(spacing: Spacing.md) {
                    runButton
                    autoButton
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
            autoMode = AutoMode(stimulus: wave.stimulus.kind)
            if let cron = wave.stimulus.cron {
                cronExpression = cron
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
                .font(.caption)
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
                .font(.system(.caption, design: .monospaced))
                .padding(Spacing.sm)
                .background(palette.surface)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
                .accessibilityLabel("Cron expression")

            Text("Examples: 0 9 * * * (daily 9am), */30 * * * * (every 30 min)")
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
    }

    // MARK: - Action Buttons

    private var buttonsDisabled: Bool {
        isSendingRun || selectedFlow.isEmpty || wave.status == .running || wave.status == .waiting
    }

    private var runButton: some View {
        Button {
            runWith(stimulus: .once)
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
            .background(buttonsDisabled ? Color.gray : palette.accent)
            .foregroundStyle(.white)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        }
        .buttonStyle(.plain)
        .disabled(buttonsDisabled)
        .opacity(buttonsDisabled ? 0.5 : 1)
    }

    private var autoButton: some View {
        HStack(spacing: 0) {
            Button {
                runWith(stimulus: autoMode.stimulusKind)
            } label: {
                HStack(spacing: Spacing.sm) {
                    Image(systemName: autoMode.icon)
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
                ForEach(AutoMode.allCases, id: \.rawValue) { kind in
                    Button { autoMode = kind } label: {
                        Label(kind.label, systemImage: kind.icon)
                    }
                }
            } label: {
                Image(systemName: "chevron.down")
                    .font(.caption2)
                    .padding(.vertical, Spacing.lg)
                    .padding(.horizontal, Spacing.sm)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Change auto mode")
        }
        .background(buttonsDisabled ? Color.gray : palette.surface)
        .foregroundStyle(buttonsDisabled ? .white : palette.text)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        .disabled(buttonsDisabled)
        .opacity(buttonsDisabled ? 0.5 : 1)
    }

    // MARK: - Actions

    private func runWith(stimulus kind: Stimulus.Kind) {
        guard !selectedFlow.isEmpty else { return }

        isSendingRun = true
        let stimulus = Stimulus(
            kind: kind,
            cron: kind == .cron ? cronExpression : nil
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
            await MainActor.run {
                isSendingRun = false
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
        .frame(width: 500, height: 600)
}
