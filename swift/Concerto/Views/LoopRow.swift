// Row view for displaying an agent in the sidebar.

import SwiftUI
import LoopflowCore

struct AgentRow: View {
    let agent: Agent
    let isSelected: Bool
    let liveOutput: [OutputLine]
    let onSelect: () -> Void

    @State private var isHovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Circle()
                    .fill(agent.status.color)
                    .frame(width: 8, height: 8)

                Text(agent.areaDisplay)
                    .fontWeight(.medium)
                    .foregroundStyle(.white)

                Spacer()

                Text(agent.statusText)
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.6))
            }

            if !agent.detailText.isEmpty {
                Text(agent.detailText)
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.6))
            }

            if !agent.iterationText.isEmpty {
                HStack(spacing: 4) {
                    Image(systemName: "arrow.triangle.2.circlepath")
                        .font(.caption2)
                        .foregroundStyle(.white.opacity(0.3))

                    Text(agent.iterationText)
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.6))
                }
            }

            // Live output when selected or running
            if (isSelected || agent.status == .running) && !liveOutput.isEmpty {
                LoopLiveOutput(lines: liveOutput)
                    .frame(height: 120)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(isSelected ? Color.white.opacity(0.2) : (isHovering ? Color.white.opacity(0.1) : Color.clear))
        )
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovering = hovering
        }
        .onTapGesture {
            onSelect()
        }
    }
}

// Backwards compatibility alias
typealias LoopRow = AgentRow
