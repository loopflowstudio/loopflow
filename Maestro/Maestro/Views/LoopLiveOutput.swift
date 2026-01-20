// Compact live output view for loops in the sidebar.

import SwiftUI

struct LoopLiveOutput: View {
    let lines: [OutputLine]

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 1) {
                    ForEach(recentLines) { line in
                        Text(line.text)
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(colorFor(line.text))
                            .lineLimit(1)
                            .truncationMode(.tail)
                            .id(line.id)
                    }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .background(Color.black.opacity(0.3))
            .clipShape(RoundedRectangle(cornerRadius: 6))
            .onChange(of: lines.count) { _, _ in
                if let lastLine = lines.last {
                    withAnimation(.easeOut(duration: 0.1)) {
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
        if text.hasPrefix("→") { return .blue }
        if text.hasPrefix("✓") { return .green }
        if text.hasPrefix("✗") { return .red }
        if text.hasPrefix("⚠") { return .orange }
        return .white.opacity(0.8)
    }
}
