// Flow picker for experimenting with flows on idle agents.
// Run different flows without changing the agent's default config.

import SwiftUI
import LoopflowCore

struct FlowPicker: View {
    let agent: Agent

    @Environment(RepoState.self) private var repoState
    @Environment(SessionState.self) private var sessionState
    @Environment(\.colorScheme) private var colorScheme

    @State private var selectedFlowName: String = ""
    @State private var isInteractive = false
    @State private var isRunning = false
    @State private var isSaving = false
    @State private var errorMessage: String?
    @State private var showingError = false

    private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Try a different flow")
                    .font(.headline)
                Spacer()
            }

            Text("Run a flow without changing the agent's default configuration.")
                .font(.caption)
                .foregroundStyle(.secondary)

            // Flow selector and run buttons
            HStack(spacing: 12) {
                // Flow picker (shows flows and steps with icons)
                Picker("Flow", selection: $selectedFlowName) {
                    // Flows section (multi-step pipelines)
                    let flows = repoState.flows.filter { $0.type == .flow }
                    if !flows.isEmpty {
                        Section("Flows") {
                            ForEach(flows) { flow in
                                Label(flow.name, systemImage: "arrow.triangle.branch")
                                    .tag(flow.name)
                            }
                        }
                    }

                    // Steps section (single-step actions)
                    let steps = repoState.flows.filter { $0.type == .step }
                    if !steps.isEmpty {
                        Section("Steps") {
                            ForEach(steps) { step in
                                Label(step.name, systemImage: "square")
                                    .tag(step.name)
                            }
                        }
                    }
                }
                .pickerStyle(.menu)
                .frame(minWidth: 140)
                .disabled(repoState.flows.isEmpty)

                // Interactive toggle
                Toggle(isOn: $isInteractive) {
                    Text("Interactive")
                        .font(.caption)
                }
                .toggleStyle(.checkbox)
                .help("Run in embedded terminal with interactive input")

                Spacer()

                // Run button
                Button {
                    if isInteractive {
                        launchInteractiveSession()
                    } else {
                        runExperiment()
                    }
                } label: {
                    HStack(spacing: 4) {
                        if isRunning {
                            ProgressView()
                                .scaleEffect(0.7)
                        } else {
                            Image(systemName: isInteractive ? "terminal" : "play.fill")
                                .font(.caption)
                        }
                        Text("Run")
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(isRunning || selectedFlowName.isEmpty || !agent.isConfigured || (isInteractive && agent.worktreePath == nil))

                // Set as default button
                if selectedFlowName != agent.flow && !selectedFlowName.isEmpty {
                    Button {
                        setAsDefault()
                    } label: {
                        HStack(spacing: 4) {
                            if isSaving {
                                ProgressView()
                                    .scaleEffect(0.7)
                            } else {
                                Image(systemName: "checkmark")
                                    .font(.caption)
                            }
                            Text("Set Default")
                        }
                    }
                    .buttonStyle(DarkButtonStyle())
                    .disabled(isSaving)
                    .help("Make '\(selectedFlowName)' the agent's default flow")
                }
            }

            // Help text showing what the flow does
            if !selectedFlowName.isEmpty {
                flowDescription
            }
        }
        .padding(16)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .alert("Error", isPresented: $showingError) {
            Button("OK") { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "Failed to run flow")
        }
        .onAppear {
            initializeSelection()
        }
        .onChange(of: repoState.flows) {
            if selectedFlowName.isEmpty {
                initializeSelection()
            }
        }
    }

    private func initializeSelection() {
        // Default to agent's configured flow, or first available
        if !agent.flow.isEmpty && repoState.flows.contains(where: { $0.name == agent.flow }) {
            selectedFlowName = agent.flow
        } else if let design = repoState.flows.first(where: { $0.name == "design" }) {
            selectedFlowName = design.name
        } else if let first = repoState.flows.first {
            selectedFlowName = first.name
        }
    }

    @ViewBuilder
    private var flowDescription: some View {
        if let flow = repoState.flows.first(where: { $0.name == selectedFlowName }) {
            if flow.type == .step {
                Text("Single step: \(flow.name)")
                    .font(.caption)
                    .foregroundStyle(.tertiary)
            } else if !flow.steps.isEmpty {
                let stepNames = flow.steps.map(\.prompt).joined(separator: " \u{2192} ")
                Text(stepNames)
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
            }
        }
    }

    private func runExperiment() {
        guard !selectedFlowName.isEmpty else { return }

        isRunning = true
        Task {
            do {
                // Run with flow override - doesn't change agent's config
                try await repoState.runAgent(
                    agent: agent,
                    flow: selectedFlowName,
                    stimulus: Stimulus(kind: .once)  // Experiments run once
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

    private func launchInteractiveSession() {
        guard !selectedFlowName.isEmpty, let path = agent.worktreePath else { return }
        sessionState.launchInteractiveSession(agentId: agent.id, step: selectedFlowName, worktreePath: path)
    }

    private func setAsDefault() {
        guard !selectedFlowName.isEmpty else { return }

        isSaving = true
        Task {
            do {
                try await repoState.updateAgent(agent, flow: selectedFlowName)
            } catch {
                await MainActor.run {
                    errorMessage = error.localizedDescription
                    showingError = true
                }
            }
            await MainActor.run {
                isSaving = false
            }
        }
    }
}

#Preview {
    let repoState = RepoState()
    repoState.configureMockAgents()
    repoState.flows = [
        Flow(name: "design", steps: [Step(prompt: "design")], type: .step),
        Flow(name: "ship", steps: [
            Step(prompt: "design"),
            Step(prompt: "code"),
            Step(prompt: "test")
        ], type: .flow)
    ]

    let agent = repoState.agents.first { $0.status == .idle } ?? repoState.agents.first!
    return FlowPicker(agent: agent)
        .environment(repoState)
        .environment(SessionState())
        .frame(width: 500)
        .padding()
}
