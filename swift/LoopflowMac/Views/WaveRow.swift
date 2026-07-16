// Row view for displaying a wave in the sidebar. One stable row: an operational
// lens, the Wave name, and a one-line objective tagline. No status pill, no
// state regrouping. `indentLevel` reserves room for future Wave ancestry.

import SwiftUI
import Loopflow

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
                            .foregroundStyle(.white.opacity(0.55))
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
                        .foregroundStyle(.white.opacity(0.5))
                        .lineLimit(1)
                        .accessibilityIdentifier("wave-objective")
                }
            }
        }
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
        .accessibilityLabel("Wave: \(wave.displayName). \(wave.lens.reason)")
        .accessibilityValue(accessibilityValue)
        .accessibilityHint(indentLevel > 0 ? "Child wave" : "Top-level wave")
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
    }
}
