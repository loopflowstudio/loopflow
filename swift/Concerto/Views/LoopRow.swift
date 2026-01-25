// Row view for displaying an agent in the sidebar.
// Uses status indicators from design: ● Running, ◐ Waiting, ○ Idle, ◷ Scheduled, ✓ Completed, ✗ Error

import SwiftUI
import LoopflowCore

struct AgentRow: View {
    let agent: Agent
    let isSelected: Bool
    var isKeyboardFocused: Bool = false
    let liveOutput: [OutputLine]
    let onSelect: () -> Void

    @State private var isHovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                // Status indicator using design system icons
                Image(systemName: agent.statusIndicator.icon)
                    .font(.system(size: 10))
                    .foregroundStyle(agent.statusIndicator.color)
                    .help(statusHelpText)

                // Display name (user-visible name, not area)
                Text(agent.displayName)
                    .fontWeight(.medium)
                    .foregroundStyle(.white)
                    .lineLimit(1)

                Spacer()

                // Flow badge
                Text(agent.flowDisplay)
                    .font(.caption2)
                    .fontWeight(.medium)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(Color.white.opacity(0.15))
                    .foregroundStyle(.white.opacity(0.7))
                    .clipShape(Capsule())
            }

            // Secondary info line
            HStack(spacing: 4) {
                Text(agent.areaDisplay)
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.5))

                if !agent.iterationText.isEmpty {
                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.3))
                    Text(agent.iterationText)
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.5))
                }

                if agent.status == .waiting {
                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.3))
                    Text("PR limit")
                        .font(.caption)
                        .foregroundStyle(.yellow.opacity(0.7))
                }

                if agent.stimulus.kind == .cron, let cron = agent.stimulus.cron {
                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.3))
                    Text(formatCron(cron))
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.5))
                }
            }

            // Live output when selected or running
            if (isSelected || agent.status == .running) && !liveOutput.isEmpty {
                LoopLiveOutput(lines: liveOutput)
                    .frame(height: 80)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(isSelected ? Color.white.opacity(0.2) : (isHovering ? Color.white.opacity(0.08) : Color.clear))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.accentColor, lineWidth: 2)
                .opacity(isKeyboardFocused && !isSelected ? 1 : 0)
        )
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovering = hovering
        }
        .onTapGesture {
            onSelect()
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Agent: \(agent.displayName)")
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
    }

    private var statusHelpText: String {
        switch agent.status {
        case .running: return "Running"
        case .waiting: return "Waiting (PR limit reached)"
        case .idle:
            if agent.stimulus.kind == .cron {
                return "Scheduled"
            }
            return "Idle"
        case .completed: return "Completed"
        case .error: return "Error"
        }
    }

    private func formatCron(_ cron: String) -> String {
        if cron.hasPrefix("0 9 * * *") {
            return "9am daily"
        } else if cron.hasPrefix("0 9 * * MON-FRI") {
            return "9am weekdays"
        }
        return cron
    }
}

// Backwards compatibility alias
typealias LoopRow = AgentRow
