// Flow picker for running flows on idle agents.
// Allows running once, or changing stimulus to loop/watch/cron.

import SwiftUI
import LoopflowCore

struct FlowPicker: View {
    let agent: Agent
    @Bindable var appState: AppState

    @Environment(\.colorScheme) private var colorScheme

    @State private var inputText: String = ""
    @State private var selectedFlowName: String = ""
    @State private var isRunning = false
    @State private var errorMessage: String?
    @State private var showingError = false
    @State private var showingScheduleSheet = false

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Run a flow")
                .font(.headline)

            // Optional description/goal input
            VStack(alignment: .leading, spacing: 6) {
                Text("What should it do?")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                TextField("Optional: describe the task...", text: $inputText)
                    .textFieldStyle(.roundedBorder)
            }

            // Flow selector and run buttons
            HStack(spacing: 12) {
                // Flow picker
                VStack(alignment: .leading, spacing: 6) {
                    Text("Flow")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    Picker("Flow", selection: $selectedFlowName) {
                        ForEach(appState.flows) { flow in
                            Text(flow.name).tag(flow.name)
                        }
                    }
                    .pickerStyle(.menu)
                    .frame(minWidth: 100)
                    .disabled(appState.flows.isEmpty)
                }

                Spacer()

                // Run buttons
                HStack(spacing: 8) {
                    Button {
                        runFlow(stimulus: .once)
                    } label: {
                        HStack(spacing: 4) {
                            if isRunning {
                                ProgressView()
                                    .scaleEffect(0.7)
                            } else {
                                Image(systemName: "play.fill")
                                    .font(.caption)
                            }
                            Text("Run Once")
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(isRunning)

                    Menu {
                        Button {
                            runFlow(stimulus: .loop)
                        } label: {
                            Label("Loop (continuous)", systemImage: "arrow.triangle.2.circlepath")
                        }

                        Button {
                            runFlow(stimulus: .watch)
                        } label: {
                            Label("Watch \(agent.areaDisplay) (on change)", systemImage: "eye")
                        }

                        Button {
                            showingScheduleSheet = true
                        } label: {
                            Label("Schedule...", systemImage: "clock")
                        }
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: "ellipsis.circle")
                                .font(.caption)
                            Text("Keep Running")
                        }
                    }
                    .menuStyle(.borderedButton)
                    .disabled(isRunning || !isConfigured)
                }
            }

            // Stimulus help text
            Text(stimulusHelpText)
                .font(.caption)
                .foregroundStyle(.tertiary)
        }
        .padding(20)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .alert("Error", isPresented: $showingError) {
            Button("OK") { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "Failed to run flow")
        }
        .sheet(isPresented: $showingScheduleSheet) {
            SchedulePickerSheet(
                onSchedule: { cron in
                    runFlowWithCron(cron)
                }
            )
        }
        .onAppear {
            if selectedFlowName.isEmpty, let first = appState.flows.first {
                selectedFlowName = first.name
            }
        }
    }

    private var stimulusHelpText: String {
        if appState.flows.isEmpty {
            return "No flows defined. Add flows to .lf/flows/ in your repo."
        }
        guard let flow = appState.flows.first(where: { $0.name == selectedFlowName }) else {
            return "Select a flow to run on this agent."
        }
        let stepNames = flow.steps.map(\.prompt).joined(separator: " → ")
        return stepNames.isEmpty ? "Run the \(flow.name) flow." : stepNames
    }

    private func runFlow(stimulus: Stimulus.Kind) {
        isRunning = true
        Task {
            do {
                try await appState.runAgentFlow(
                    agent: agent,
                    flow: selectedFlowName,
                    stimulus: stimulus,
                    args: inputText
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

    private func runFlowWithCron(_ cron: String) {
        isRunning = true
        Task {
            do {
                // For cron, we need to set the stimulus with the cron expression
                // The runAgentFlow should handle this via the args or a separate parameter
                try await appState.runAgentFlow(
                    agent: agent,
                    flow: selectedFlowName,
                    stimulus: .cron,
                    args: inputText.isEmpty ? cron : "\(inputText) --cron '\(cron)'"
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

// MARK: - Schedule Picker Sheet

struct SchedulePickerSheet: View {
    @Environment(\.dismiss) private var dismiss
    let onSchedule: (String) -> Void

    @State private var selectedPreset: CronPreset = .daily9am

    enum CronPreset: String, CaseIterable {
        case daily9am = "0 9 * * *"
        case weekdays9am = "0 9 * * MON-FRI"
        case hourly = "0 * * * *"
        case every6hours = "0 */6 * * *"

        var displayName: String {
            switch self {
            case .daily9am: return "Daily at 9am"
            case .weekdays9am: return "Weekdays at 9am"
            case .hourly: return "Every hour"
            case .every6hours: return "Every 6 hours"
            }
        }
    }

    var body: some View {
        VStack(spacing: 20) {
            Text("Schedule Agent")
                .font(.headline)

            VStack(alignment: .leading, spacing: 12) {
                Text("Run this agent on a schedule:")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)

                ForEach(CronPreset.allCases, id: \.self) { preset in
                    Button {
                        selectedPreset = preset
                    } label: {
                        HStack {
                            Image(systemName: selectedPreset == preset ? "checkmark.circle.fill" : "circle")
                                .foregroundStyle(selectedPreset == preset ? .blue : .secondary)
                            Text(preset.displayName)
                            Spacer()
                            Text(preset.rawValue)
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                        }
                        .padding(.vertical, 4)
                    }
                    .buttonStyle(.plain)
                }
            }

            HStack {
                Button("Cancel") {
                    dismiss()
                }
                .keyboardShortcut(.escape)

                Spacer()

                Button("Schedule") {
                    onSchedule(selectedPreset.rawValue)
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 360)
    }
}

#Preview {
    let state = AppState()
    state.configureMockAgents()
    let agent = state.agents.first { $0.status == .idle } ?? state.agents.first!
    return FlowPicker(agent: agent, appState: state)
        .frame(width: 500)
        .padding()
}
