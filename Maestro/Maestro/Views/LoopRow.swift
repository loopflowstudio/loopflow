// Row view for displaying a loop in the sidebar.

import SwiftUI

struct LoopRow: View {
    let loop: Loop
    let isSelected: Bool
    let liveOutput: [OutputLine]
    let hasLandableWork: Bool
    let onSelect: () -> Void
    let onLand: () -> Void

    @State private var isHovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Circle()
                    .fill(loop.status.color)
                    .frame(width: 8, height: 8)

                Text(loop.areaDisplay)
                    .fontWeight(.medium)
                    .foregroundStyle(.white)

                Spacer()

                if isHovering && hasLandableWork {
                    Button {
                        onLand()
                    } label: {
                        Text("Land")
                            .font(.caption)
                            .foregroundStyle(.white.opacity(0.7))
                    }
                    .buttonStyle(.plain)
                    .help("Squash and land to main")
                } else {
                    Text(loop.statusText)
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.6))
                }
            }

            if !loop.detailText.isEmpty {
                Text(loop.detailText)
                    .font(.caption)
                    .foregroundStyle(.white.opacity(0.6))
            }

            if !loop.iterationText.isEmpty {
                HStack(spacing: 4) {
                    Image(systemName: "arrow.triangle.2.circlepath")
                        .font(.caption2)
                        .foregroundStyle(.white.opacity(0.3))

                    Text(loop.iterationText)
                        .font(.caption)
                        .foregroundStyle(.white.opacity(0.6))
                }
            }

            // Live output when selected or running
            if (isSelected || loop.status == .running) && !liveOutput.isEmpty {
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
