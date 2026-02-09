// Row view for displaying a wave in the sidebar.

import SwiftUI
import AppKit
import LoopflowCore

struct WaveRow: View {
    let wave: WaveViewModel
    let isSelected: Bool
    var isKeyboardFocused: Bool = false
    let onSelect: () -> Void
    var onDelete: (() -> Void)? = nil
    var onRename: ((String) -> Void)? = nil
    @Binding var isEditingAnyName: Bool

    @State private var isHovering = false
    @State private var isEditingName = false
    @State private var editingName = ""
    @FocusState private var isNameFocused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                // Display name (click to edit)
                if isEditingName {
                    TextField("Wave name", text: $editingName)
                        .fontWeight(.medium)
                        .foregroundStyle(.white)
                        .textFieldStyle(.plain)
                        .focused($isNameFocused)
                        .onSubmit { commitNameEdit() }
                        .onExitCommand { cancelNameEdit() }
                        .accessibilityIdentifier("wave-name-edit")
                } else {
                    Text(wave.displayName)
                        .fontWeight(.medium)
                        .foregroundStyle(.white)
                        .lineLimit(1)
                        .accessibilityIdentifier("wave-name")
                        .onTapGesture {
                            guard isSelected else { return }
                            startNameEdit()
                        }
                }

                Spacer()

                // PR badge (if pending review) or Flow badge
                if let pr = wave.pendingPR {
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
                            .background(Color.statusSuccess.opacity(0.3))
                            .foregroundStyle(Color.statusSuccess)
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

                // Activity timestamp (italic serif per VISUAL_DESIGN.md)
                if let activity = wave.lastActivityDescription {
                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.3))
                    Text(activity)
                        .font(.custom("Cormorant Garamond", size: 11))
                        .italic()
                        .foregroundStyle(.white.opacity(0.4))
                        .accessibilityLabel(activityAccessibilityLabel)
                        .accessibilityIdentifier("wave-activity")
                }

                if wave.status == .waiting {
                    Text("•")
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.3))
                    Text("PR limit")
                        .font(.caption)
                        .foregroundStyle(Color.statusWarning.opacity(0.7))
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

            // Live output when selected (isolated observation scope)
            if isSelected {
                SidebarLiveOutput(waveId: wave.id)
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
        .onChange(of: isNameFocused) { _, focused in
            if !focused && isEditingName {
                commitNameEdit()
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .editWaveName)) { _ in
            if isSelected {
                startNameEdit()
            }
        }
    }

    // MARK: - Name Editing

    private func startNameEdit() {
        editingName = wave.name
        isEditingName = true
        isEditingAnyName = true
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(50))
            isNameFocused = true
        }
    }

    private func commitNameEdit() {
        let newName = editingName.trimmingCharacters(in: .whitespacesAndNewlines)
        isEditingName = false
        isEditingAnyName = false
        isNameFocused = false
        guard !newName.isEmpty, newName != wave.name else { return }
        onRename?(newName)
    }

    private func cancelNameEdit() {
        isEditingName = false
        isEditingAnyName = false
        isNameFocused = false
    }

    private func formatCron(_ cron: String) -> String {
        if cron.hasPrefix("0 9 * * *") {
            return "9am daily"
        } else if cron.hasPrefix("0 9 * * MON-FRI") {
            return "9am weekdays"
        }
        return cron
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

/// Isolated view for sidebar live output. Has its own observation scope
/// so output changes only re-render this view, not the entire sidebar.
private struct SidebarLiveOutput: View {
    let waveId: String
    @Environment(OutputBuffer.self) private var outputBuffer

    var body: some View {
        let lines = outputBuffer.output(for: waveId)
        if !lines.isEmpty {
            LiveOutput(lines: lines)
                .frame(height: 80)
        }
    }
}
