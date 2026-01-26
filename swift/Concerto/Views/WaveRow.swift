// Row view for displaying an wave in the sidebar.
// Uses status indicators from design: ● Running, ◐ Waiting, ○ Idle, ◷ Scheduled, ✓ Completed, ✗ Error

import SwiftUI
import AppKit
import LoopflowCore

struct WaveRow: View {
    let wave: Wave
    let isSelected: Bool
    var isKeyboardFocused: Bool = false
    let liveOutput: [OutputLine]
    var pendingPR: (number: Int, url: URL?)? = nil  // PR awaiting review
    let onSelect: () -> Void

    @State private var isHovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                // Status indicator using design system icons
                Image(systemName: wave.statusIndicator.icon)
                    .font(.system(size: 10))
                    .foregroundStyle(wave.statusIndicator.color)
                    .help(statusHelpText)
                    .accessibilityIdentifier("wave-status")

                // Display name (user-visible name, not area)
                Text(wave.displayName)
                    .fontWeight(.medium)
                    .foregroundStyle(.white)
                    .lineLimit(1)
                    .accessibilityIdentifier("wave-name")

                Spacer()

                // PR badge (if pending review) or Flow badge
                if let pr = pendingPR {
                    Button {
                        if let url = pr.url {
                            NSWorkspace.shared.open(url)
                        }
                    } label: {
                        Text("PR #\(pr.number)")
                            .font(.caption2)
                            .fontWeight(.medium)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.green.opacity(0.3))
                            .foregroundStyle(.green)
                            .clipShape(Capsule())
                    }
                    .buttonStyle(.plain)
                } else {
                    Text(wave.flowDisplay)
                        .font(.caption2)
                        .fontWeight(.medium)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.white.opacity(0.15))
                        .foregroundStyle(.white.opacity(0.7))
                        .clipShape(Capsule())
                        .accessibilityIdentifier("wave-flow")
                }
            }

            // Secondary info line
            HStack(spacing: 4) {
                Text(wave.areaDisplay)
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.5))
                    .accessibilityIdentifier("wave-area")

                if !wave.iterationText.isEmpty {
                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.3))
                    Text(wave.iterationText)
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.5))
                        .accessibilityIdentifier("wave-iteration")
                }

                if wave.status == .waiting {
                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.3))
                    Text("PR limit")
                        .font(.caption)
                        .foregroundStyle(.yellow.opacity(0.7))
                        .accessibilityIdentifier("wave-pr-limit")
                }

                if wave.stimulus.kind == .cron, let cron = wave.stimulus.cron {
                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.3))
                    Text(formatCron(cron))
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.5))
                        .accessibilityIdentifier("wave-cron")
                }
            }

            // Live output when selected or running
            if (isSelected || wave.status == .running) && !liveOutput.isEmpty {
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
        .accessibilityLabel("Wave: \(wave.displayName)")
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
    }

    private var statusHelpText: String {
        switch wave.status {
        case .running: return "Running"
        case .waiting: return "Waiting (PR limit reached)"
        case .idle:
            if wave.stimulus.kind == .cron {
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
