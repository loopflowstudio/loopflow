// Row view for displaying a wave in the sidebar.

import SwiftUI
import Loopflow

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
                    .font(Typography.sectionTitle(18))
                    .fontWeight(.semibold)
                    .foregroundStyle(.white)
                    .lineLimit(1)
                    .accessibilityIdentifier("wave-name")

                Spacer()

                Text(wave.statusText)
                    .font(Typography.caption(10))
                    .fontWeight(.medium)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(wave.statusIndicator.color.opacity(0.22))
                    .foregroundStyle(wave.statusIndicator.color)
                    .clipShape(Capsule())
                    .fixedSize()
                    .accessibilityIdentifier("wave-status")
            }
            .lineLimit(1)
            .accessibilityIdentifier("wave-name-row")

            if let tagline = wave.objectiveTagline {
                Text(tagline)
                    .font(Typography.caption())
                    .foregroundStyle(.white.opacity(0.5))
                    .lineLimit(1)
                    .accessibilityIdentifier("wave-objective")
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
}
