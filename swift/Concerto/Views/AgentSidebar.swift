// Unified sidebar showing all agents.

import SwiftUI
import LoopflowCore

struct AgentSidebar: View {
    @Environment(RepoState.self) private var repoState
    @Environment(SessionState.self) private var sessionState

    @State private var showingNewAgentSheet = false
    @State private var showingDiagnostics = false
    @State private var actionError: String?
    @State private var showingActionError = false

    // Keyboard navigation state
    @State private var keyboardFocusedId: String?
    @FocusState private var isSidebarFocused: Bool

    // MARK: - Agent Sections

    private var blockedAgents: [Agent] {
        repoState.agents.filter { $0.status == .error }
    }

    private var prAgents: [Agent] {
        repoState.agents.filter { agent in
            agent.status != .error && pendingPR(for: agent) != nil
        }
    }

    private var activeAgents: [Agent] {
        repoState.agents.filter { agent in
            (agent.status == .running || agent.status == .waiting) &&
            agent.status != .error &&
            pendingPR(for: agent) == nil
        }
    }

    private var idleAgents: [Agent] {
        repoState.agents.filter { agent in
            agent.status == .idle && pendingPR(for: agent) == nil
        }
    }

    private var allAgentsInOrder: [Agent] {
        blockedAgents + prAgents + activeAgents + idleAgents
    }

    private func pendingPR(for agent: Agent) -> (number: Int, url: URL?)? {
        guard let prNumber = agent.prNumber,
              agent.prState == .open else {
            return nil
        }
        return (number: prNumber, url: agent.prURL)
    }

    private func sectionHeader(_ title: String, icon: String, color: Color) -> some View {
        HStack(spacing: 6) {
            Image(systemName: icon)
                .font(.system(size: 9))
                .foregroundStyle(color)
            Text(title)
                .font(.caption2)
                .fontWeight(.medium)
                .foregroundStyle(.white.opacity(0.6))
        }
        .padding(.leading, 8)
        .padding(.top, 12)
        .padding(.bottom, 4)
    }

    private func agentRows(_ agents: [Agent]) -> some View {
        ForEach(agents) { agent in
            AgentRow(
                agent: agent,
                isSelected: repoState.selectedAgent?.id == agent.id,
                isKeyboardFocused: keyboardFocusedId == agent.id,
                liveOutput: sessionState.output(for: agent.id),
                pendingPR: pendingPR(for: agent),
                onSelect: {
                    repoState.selectedAgent = agent
                    keyboardFocusedId = agent.id
                }
            )
        }
    }

    var body: some View {
        @Bindable var repoState = repoState
        VStack(alignment: .leading, spacing: 0) {
            header

            if repoState.agents.isEmpty && !repoState.lfdConnected {
                disconnectedState
            } else if repoState.agents.isEmpty {
                emptyState
            } else {
                agentList
            }
        }
        .background(Color.loopflowBurgundy)
        .sheet(isPresented: $showingNewAgentSheet) {
            NewAgentSheet()
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

            Circle()
                .fill(repoState.lfdConnected ? Color.green : Color.white.opacity(0.3))
                .frame(width: 6, height: 6)
                .help(repoState.lfdConnected ? "Connected to lfd daemon" : "lfd daemon not connected")

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
                            try await repoState.connectLfd()
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
        @Bindable var repoState = repoState
        return ScrollView {
            LazyVStack(alignment: .leading, spacing: 4) {
                if !blockedAgents.isEmpty {
                    sectionHeader("Needs Attention", icon: "exclamationmark.triangle.fill", color: .orange)
                    agentRows(blockedAgents)
                }

                if !prAgents.isEmpty {
                    sectionHeader("Open PRs", icon: "arrow.triangle.pull", color: .green)
                    agentRows(prAgents)
                }

                if !activeAgents.isEmpty {
                    sectionHeader("Active", icon: "circle.fill", color: .blue)
                    agentRows(activeAgents)
                }

                if !idleAgents.isEmpty {
                    sectionHeader("Idle", icon: "circle", color: .white.opacity(0.5))
                    agentRows(idleAgents)
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
               let agent = repoState.agents.first(where: { $0.id == id }) {
                repoState.selectedAgent = agent
            }
            return .handled
        }
    }

    private func moveFocus(_ delta: Int) {
        let agents = allAgentsInOrder
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
    let state = RepoState()
    state.configureMockAgents()
    return AgentSidebar()
        .environment(state)
        .environment(SessionState())
        .frame(width: 280, height: 400)
}
