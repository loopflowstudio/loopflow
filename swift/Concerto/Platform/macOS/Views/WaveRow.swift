// Row view for displaying a wave in the sidebar.

import SwiftUI
import AppKit
import LoopflowCore

struct WaveRow: View {
    let wave: WaveViewModel
    let isSelected: Bool
    let onSelect: () -> Void
    var onDelete: (() -> Void)? = nil

    @State private var isHovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                Text(wave.displayName)
                    .fontWeight(.medium)
                    .foregroundStyle(.white)
                    .lineLimit(1)
                    .accessibilityIdentifier("wave-name")

                Spacer()

                HStack(spacing: 6) {
                    // PR badge
                    if let pr = wave.pendingPR {
                        Button {
                            if let url = pr.url {
                                NSWorkspace.shared.open(url)
                            }
                        } label: {
                            Text("PR #\(pr.number)")
                                .font(Typography.caption(10))
                                .fontWeight(.medium)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Color.statusSuccess.opacity(0.3))
                                .foregroundStyle(Color.statusSuccess)
                                .clipShape(Capsule())
                        }
                        .buttonStyle(.plain)
                    }

                    // Diff indicator
                    if let diff = wave.diffIndicator {
                        Text(diff)
                            .font(Typography.caption(10))
                            .fontWeight(.medium)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background((wave.diffIsPositive ? Color.statusSuccess : Color.statusError).opacity(0.22))
                            .foregroundStyle(wave.diffIsPositive ? Color.statusSuccess : Color.statusError)
                            .clipShape(Capsule())
                            .accessibilityIdentifier("wave-diff")
                    }

                    if wave.effectiveOpenPRCount > 1 {
                        Text("\(wave.effectiveOpenPRCount) open")
                            .font(Typography.caption(10))
                            .fontWeight(.medium)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.statusWarning.opacity(0.22))
                            .foregroundStyle(Color.statusWarning)
                            .clipShape(Capsule())
                    }
                }
                .fixedSize()
            }
            .lineLimit(1)
            .accessibilityIdentifier("wave-name-row")

            // Secondary info line: vision tagline + activity
            HStack(spacing: 4) {
                if let tagline = wave.visionTagline {
                    Text(tagline)
                        .font(Typography.caption())
                        .foregroundStyle(.white.opacity(0.5))
                        .lineLimit(1)
                        .accessibilityIdentifier("wave-vision")
                }

                // Activity timestamp (italic serif per VISUAL_DESIGN.md)
                if let activity = wave.lastActivityDescription {
                    if wave.visionTagline != nil {
                        Text("•")
                            .font(Typography.caption())
                            .foregroundStyle(.white.opacity(0.3))
                    }
                    Text(activity)
                        .font(Typography.caption(11))
                        .italic()
                        .foregroundStyle(.white.opacity(0.4))
                        .accessibilityLabel(activityAccessibilityLabel)
                        .accessibilityIdentifier("wave-activity")
                }
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .fill(isSelected ? Color.white.opacity(0.2) : (isHovering ? Color.white.opacity(0.08) : Color.clear))
        )
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovering = hovering
        }
        .onChange(of: isSelected) { _, selected in
            if selected { isHovering = false }
        }
        .onTapGesture {
            onSelect()
        }
        .contextMenu {
            if let onDelete {
                Button(role: .destructive) {
                    onDelete()
                } label: {
                    Label("Delete Wave", systemImage: "trash")
                }
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Wave: \(wave.displayName)")
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
    }

    /// Accessibility-friendly description of activity (e.g., "implement, 2 minutes ago").
    private var activityAccessibilityLabel: String {
        guard let step = wave.recentSteps.first else { return "" }
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .full
        let time = formatter.localizedString(for: step.endedAt ?? step.startedAt, relativeTo: Date())
        return "\(step.step), \(time)"
    }
}
