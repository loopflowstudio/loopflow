// Agent list sidebar for AgentWindow.

import SwiftUI

struct AgentSidebar: View {
    let agents: [Agent]
    @Binding var selected: Agent?
    let onAdd: () -> Void
    let onRefresh: () -> Void

    var body: some View {
        List(selection: $selected) {
            ForEach(agents) { agent in
                agentRow(agent)
                    .tag(agent)
            }
        }
        .listStyle(.sidebar)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button(action: onAdd) {
                    Image(systemName: "plus")
                }
                .help("New Agent")
            }

            ToolbarItem(placement: .automatic) {
                Button(action: onRefresh) {
                    Image(systemName: "arrow.clockwise")
                }
                .help("Refresh")
            }
        }
        .navigationSplitViewColumnWidth(min: 180, ideal: 220, max: 280)
    }

    private func agentRow(_ agent: Agent) -> some View {
        HStack(spacing: 8) {
            Text(agent.displayEmoji)
                .font(.title3)

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 4) {
                    Text(agent.name)
                        .fontWeight(.medium)
                        .lineLimit(1)

                    statusDot(agent.status)
                }

                Text(agent.repoName)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Spacer()
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
    }

    private func statusDot(_ status: AgentStatus) -> some View {
        Circle()
            .fill(status.color)
            .frame(width: 6, height: 6)
    }
}
