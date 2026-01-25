// Unified sidebar showing all agents. Replaces WorktreeSidebar.

import SwiftUI
import LoopflowCore

struct AgentSidebar: View {
    @Bindable var appState: AppState
    @State private var showingNewAgentSheet = false
    @State private var showingDiagnostics = false
    @State private var actionError: String?
    @State private var showingActionError = false

    // Keyboard navigation state
    @State private var keyboardFocusedId: String?
    @FocusState private var isSidebarFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header

            if appState.agents.isEmpty && !appState.lfdConnected {
                disconnectedState
            } else if appState.agents.isEmpty {
                emptyState
            } else {
                agentList
            }
        }
        .background(Color.loopflowBurgundy)
        .sheet(isPresented: $showingNewAgentSheet) {
            NewAgentSheet(appState: appState)
        }
        .sheet(isPresented: $showingDiagnostics) {
            DiagnosticsView()
        }
        .alert("Error", isPresented: $showingActionError) {
            Button("OK") { actionError = nil }
        } message: {
            Text(actionError ?? "An error occurred")
        }
        .onReceive(NotificationCenter.default.publisher(for: .showNewAgentSheet)) { _ in
            showingNewAgentSheet = true
        }
    }

    private var header: some View {
        HStack {
            Text("Agents")
                .font(.caption)
                .fontWeight(.medium)
                .foregroundStyle(.white.opacity(0.7))
                .help("Agents are autonomous AI workers that run flows on your codebase")

            Spacer()

            // lfd connection indicator
            Circle()
                .fill(appState.lfdConnected ? Color.green : Color.white.opacity(0.3))
                .frame(width: 6, height: 6)
                .help(appState.lfdConnected ? "Connected to lfd daemon" : "lfd daemon not connected")

            Button {
                showingNewAgentSheet = true
            } label: {
                Image(systemName: "plus")
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.7))
            }
            .buttonStyle(.plain)
            .help("Create a new agent")
            .accessibleButton("Create new agent")
            .minHitTarget()

            Button {
                showingDiagnostics = true
            } label: {
                Image(systemName: "doc.text.magnifyingglass")
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.7))
            }
            .buttonStyle(.plain)
            .help("Open diagnostics")
            .accessibleButton("Open diagnostics")
            .minHitTarget()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var disconnectedState: some View {
        VStack(spacing: 0) {
            Spacer()
                .frame(maxHeight: .infinity)

            VStack(spacing: 12) {
                Image(systemName: "link.circle")
                    .font(.system(size: 28))
                    .foregroundStyle(.white.opacity(0.3))

                VStack(spacing: 4) {
                    Text("Connect to lfd")
                        .fontWeight(.medium)
                        .foregroundStyle(.white.opacity(0.7))
                    Text("Start the daemon to manage agents.")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.5))
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 16)
                }

                Button {
                    Task {
                        do {
                            try await appState.connectLfd()
                        } catch {
                            actionError = "Failed to connect: \(error.localizedDescription)"
                            showingActionError = true
                        }
                    }
                } label: {
                    Label("Connect lfd", systemImage: "link")
                        .font(.caption)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
            }

            Spacer()
                .frame(maxHeight: .infinity)
            Spacer()
                .frame(maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity)
        .padding()
    }

    private var emptyState: some View {
        VStack(spacing: 0) {
            Spacer()
                .frame(maxHeight: .infinity)

            VStack(spacing: 12) {
                Image(systemName: "cpu")
                    .font(.system(size: 28))
                    .foregroundStyle(.white.opacity(0.3))

                VStack(spacing: 4) {
                    Text("No agents yet")
                        .fontWeight(.medium)
                        .foregroundStyle(.white.opacity(0.7))
                        .accessibilityIdentifier("agent-empty-title")
                    Text("Create an agent to start AI-powered work on your codebase.")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.5))
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 16)
                        .accessibilityIdentifier("agent-empty-description")
                }

                Button {
                    showingNewAgentSheet = true
                } label: {
                    Label("Create Agent", systemImage: "plus")
                        .font(.caption)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .accessibilityIdentifier("agent-empty-create")
            }

            Spacer()
                .frame(maxHeight: .infinity)
            Spacer()
                .frame(maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity)
        .padding()
    }

    private var agentList: some View {
        ScrollView {
            LazyVStack(spacing: 4) {
                ForEach(appState.agents) { agent in
                    AgentRow(
                        agent: agent,
                        isSelected: appState.selectedAgent?.id == agent.id,
                        isKeyboardFocused: keyboardFocusedId == agent.id,
                        liveOutput: appState.liveOutput(for: agent),
                        onSelect: {
                            appState.selectedAgent = agent
                            keyboardFocusedId = agent.id
                        }
                    )
                }
            }
            .padding(.horizontal, 8)
        }
        .focusable()
        .focused($isSidebarFocused)
        .focusEffectDisabled()
        .onKeyPress(.upArrow) {
            moveFocus(-1)
            return .handled
        }
        .onKeyPress(.downArrow) {
            moveFocus(1)
            return .handled
        }
        .onKeyPress(.return) {
            if let id = keyboardFocusedId,
               let agent = appState.agents.first(where: { $0.id == id }) {
                appState.selectedAgent = agent
            }
            return .handled
        }
    }

    private func moveFocus(_ delta: Int) {
        let agents = appState.agents
        guard !agents.isEmpty else { return }

        if let currentId = keyboardFocusedId,
           let currentIndex = agents.firstIndex(where: { $0.id == currentId }) {
            let newIndex = max(0, min(agents.count - 1, currentIndex + delta))
            keyboardFocusedId = agents[newIndex].id
        } else {
            keyboardFocusedId = delta > 0 ? agents.first?.id : agents.last?.id
        }
    }
}

// MARK: - Notification Names

extension Notification.Name {
    static let showNewAgentSheet = Notification.Name("showNewAgentSheet")
}

#Preview {
    let state = AppState()
    state.configureMockAgents()
    return AgentSidebar(appState: state)
        .frame(width: 280, height: 400)
}
