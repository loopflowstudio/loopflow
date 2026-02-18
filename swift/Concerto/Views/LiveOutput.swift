// LiveOutput - streaming read-only output view for agent runs.

import SwiftUI
import LoopflowCore

struct LiveOutput: View {
    let lines: [OutputLine]
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 1) {
                    ForEach(recentLines) { line in
                        Text(line.text)
                            .font(Typography.code(10))
                            .foregroundStyle(colorFor(line.text))
                            .lineLimit(1)
                            .truncationMode(.tail)
                            .id(line.id)
                    }
                }
                .padding(.horizontal, Spacing.sm)
                .padding(.vertical, Spacing.xs)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .background(Color.black.opacity(0.3))
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.sm))
            .onChange(of: lines.count) { _, _ in
                if let lastLine = lines.last {
                    withAnimation(DesignAnimation.fast(reduceMotion)) {
                        proxy.scrollTo(lastLine.id, anchor: .bottom)
                    }
                }
            }
        }
    }

    private var recentLines: [OutputLine] {
        // Show last 20 lines for compact view
        Array(lines.suffix(20))
    }

    private func colorFor(_ text: String) -> Color {
        // Parsed stream format from lfd
        if text.hasPrefix("->") { return .white.opacity(0.45) }
        if text == "ok" || text.hasPrefix("ok (") { return Color.statusSuccess }
        if text == "failed" { return Color.statusError }
        // Legacy prefix format
        if text.hasPrefix("→") { return .statusInfo }
        if text.hasPrefix("✓") { return Color.statusSuccess }
        if text.hasPrefix("✗") { return Color.statusError }
        if text.hasPrefix("⚠") { return .statusWarning }
        return .white.opacity(0.8)
    }
}
