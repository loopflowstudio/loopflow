// Row view for displaying a wave in the sidebar. One stable row: an operational
// lens, the Wave name, and a one-line objective tagline. No status pill, no
// state regrouping. `indentLevel` reserves room for future Wave ancestry.

import SwiftUI
import Loopflow

func waveOutline(_ waves: [WaveViewModel]) -> [(wave: WaveViewModel, indent: Int)] {
    let byId = Dictionary(waves.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
    let roots = waves.filter { wave in
        guard let parent = wave.parentWaveId else { return true }
        return byId[parent] == nil
    }
    let children = Dictionary(
        grouping: waves.filter { !roots.contains($0) },
        by: { $0.parentWaveId ?? "" }
    )
    func sorted(_ waves: [WaveViewModel]) -> [WaveViewModel] {
        waves.sorted {
            $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending
        }
    }
    var result: [(WaveViewModel, Int)] = []
    func append(_ wave: WaveViewModel, indent: Int) {
        result.append((wave, indent))
        for child in sorted(children[wave.id] ?? []) {
            append(child, indent: indent + 1)
        }
    }
    for root in sorted(roots) {
        append(root, indent: 0)
    }
    return result
}

struct WaveRow: View {
    let wave: WaveViewModel
    let isSelected: Bool
    let onSelect: () -> Void
    var indentLevel: Int = 0
    var onDelete: (() -> Void)? = nil

    @State private var isHovering = false

    private var accessibilityValue: String {
        let hierarchy = indentLevel > 0 ? "child wave" : "top-level wave"
        return "\(wave.lens.color.rawValue) lens; \(hierarchy)"
    }

    var body: some View {
        Button(action: onSelect) {
            WaveRowLabel(wave: wave)
        }
        .buttonStyle(.plain)
        .padding(.leading, Spacing.md + CGFloat(indentLevel) * Spacing.lg)
        .padding(.trailing, Spacing.md)
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
        .accessibilityLabel("Wave: \(wave.displayName). \(wave.lens.reason)")
        .accessibilityValue(accessibilityValue)
        .accessibilityHint(indentLevel > 0 ? "Child wave" : "Top-level wave")
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
    }
}

private struct WaveRowLabel: View {
    let wave: WaveViewModel

    var body: some View {
        HStack(alignment: .top, spacing: Spacing.sm) {
            WaveLensView(lens: wave.lens)
                .padding(.top, 3)
                .accessibilityIdentifier("wave-status")

            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: Spacing.sm) {
                    Text(wave.displayName)
                        .font(Typography.sectionTitle(18))
                        .fontWeight(.semibold)
                        .foregroundStyle(.white)
                        .lineLimit(1)
                        .accessibilityIdentifier("wave-name")

                    if wave.openTaskCount > 0 {
                        Text("\(wave.openTaskCount)")
                            .font(Typography.caption(10))
                            .fontWeight(.medium)
                            .foregroundStyle(.white.opacity(0.68))
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1)
                            .background(Color.white.opacity(0.1))
                            .clipShape(Capsule())
                            .accessibilityLabel("\(wave.openTaskCount) open tasks")
                    }

                    Spacer(minLength: 0)
                }

                if let tagline = wave.objectiveTagline {
                    Text(tagline)
                        .font(Typography.caption())
                        .foregroundStyle(.white.opacity(0.68))
                        .lineLimit(1)
                        .accessibilityIdentifier("wave-objective")
                }
            }
        }
    }
}
