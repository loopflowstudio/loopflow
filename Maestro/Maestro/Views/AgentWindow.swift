// Agent management window with sidebar + detail layout.

import SwiftUI

struct AgentWindow: View {
    @State private var agents: [Agent] = []
    @State private var selectedAgent: Agent?
    @State private var showingNewAgent = false
    @State private var isLoading = false
    @State private var errorMessage: String?

    private let agentService = AgentService()

    var body: some View {
        NavigationSplitView {
            AgentSidebar(
                agents: agents,
                selected: $selectedAgent,
                onAdd: { showingNewAgent = true },
                onRefresh: { Task { await refresh() } }
            )
        } detail: {
            if let agent = selectedAgent {
                AgentDetailPanel(
                    agent: agent,
                    onSave: { updated in
                        Task { await save(updated) }
                    },
                    onStart: {
                        Task { await start(agent) }
                    },
                    onStop: {
                        Task { await stop(agent) }
                    },
                    onDelete: {
                        Task { await delete(agent) }
                    }
                )
            } else {
                ContentUnavailableView {
                    Label("No Agent Selected", systemImage: "cpu")
                } description: {
                    Text("Select an agent from the sidebar or create a new one.")
                }
            }
        }
        .navigationTitle("Agents")
        .task {
            await refresh()
        }
        .sheet(isPresented: $showingNewAgent) {
            NewAgentSheet { name, emoji, repo in
                Task { await create(name: name, emoji: emoji, repo: repo) }
            }
        }
        .alert("Error", isPresented: .constant(errorMessage != nil)) {
            Button("OK") { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "")
        }
    }

    private func refresh() async {
        isLoading = true
        do {
            agents = try await agentService.list()
            // Update selection if still valid
            if let selected = selectedAgent {
                selectedAgent = agents.first { $0.id == selected.id }
            }
        } catch {
            errorMessage = error.localizedDescription
        }
        isLoading = false
    }

    private func create(name: String, emoji: String, repo: URL) async {
        do {
            try agentService.create(name: name, emoji: emoji, repo: repo)
            await refresh()
            selectedAgent = agents.first { $0.id == name }
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func save(_ agent: Agent) async {
        do {
            try agentService.save(agent)
            await refresh()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func start(_ agent: Agent) async {
        do {
            try await agentService.start(agentId: agent.id, repoRoot: URL(fileURLWithPath: agent.repo))
            await refresh()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func stop(_ agent: Agent) async {
        do {
            try await agentService.stop(agentId: agent.id)
            await refresh()
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private func delete(_ agent: Agent) async {
        do {
            try agentService.delete(agent)
            selectedAgent = nil
            await refresh()
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
