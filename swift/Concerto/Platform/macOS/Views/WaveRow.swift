// Row view for displaying a wave in the sidebar.

import SwiftUI
import AppKit
import LoopflowCore

struct WaveRow: View {
    let wave: WaveViewModel
    let isSelected: Bool
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

                HStack(spacing: 6) {
                    // PR badge (if pending review) or Flow badge
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
                    } else if !wave.flow.isEmpty {
                        Text(wave.flow)
                            .font(Typography.caption(10))
                            .fontWeight(.medium)
                            .padding(.horizontal, 6)
                            .padding(.vertical, 2)
                            .background(Color.white.opacity(0.15))
                            .foregroundStyle(.white.opacity(0.7))
                            .clipShape(Capsule())
                            .accessibilityIdentifier("wave-flow")
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

            // Secondary info line
            HStack(spacing: 4) {
                Text(wave.areaDisplay)
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.5))
                    .accessibilityIdentifier("wave-area")

                // Activity timestamp (italic serif per VISUAL_DESIGN.md)
                if let activity = wave.lastActivityDescription {
                    Text("•")
                        .font(Typography.caption())
                        .foregroundStyle(.white.opacity(0.3))
                    Text(activity)
                        .font(Typography.caption(11))
                        .italic()
                        .foregroundStyle(.white.opacity(0.4))
                        .accessibilityLabel(activityAccessibilityLabel)
                        .accessibilityIdentifier("wave-activity")
                }

                if let triggerLabel {
                    Text("•")
                        .font(Typography.caption())
                        .foregroundStyle(.white.opacity(0.3))
                    Text(triggerLabel)
                        .font(Typography.caption())
                        .foregroundStyle(.white.opacity(0.5))
                        .accessibilityIdentifier("wave-trigger")
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

    private var triggerLabel: String? {
        guard let trigger = wave.trigger else { return nil }
        switch trigger.signal {
        case .repo: return "repo"
        case .wave: return "wave"
        case .block: return "block"
        case .ciFailure: return "ci-fix"
        }
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
