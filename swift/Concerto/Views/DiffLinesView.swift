import SwiftUI
import LoopflowCore

// MARK: - Model

struct DiffLine: Identifiable {
    let id: Int
    let text: String
    let kind: DiffLineKind
}

enum DiffLineKind {
    case addition
    case deletion
    case hunk
    case header
    case context
}

// MARK: - Parsing

func parseDiffLines(_ diff: String) -> [DiffLine] {
    guard !diff.isEmpty else { return [] }

    return diff.split(separator: "\n", omittingEmptySubsequences: false)
        .enumerated()
        .map { index, line in
            let text = String(line)
            let kind: DiffLineKind
            if text.hasPrefix("+++") || text.hasPrefix("---") {
                kind = .header
            } else if text.hasPrefix("@@") {
                kind = .hunk
            } else if text.hasPrefix("+") {
                kind = .addition
            } else if text.hasPrefix("-") {
                kind = .deletion
            } else {
                kind = .context
            }
            return DiffLine(id: index, text: text, kind: kind)
        }
}

// MARK: - View

struct DiffLinesView: View {
    @Environment(\.palette) private var palette
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let diff: String

    @State private var isHovering = false
    @State private var didCopy = false

    private var lines: [DiffLine] { parseDiffLines(diff) }

    private var additionCount: Int { lines.filter { $0.kind == .addition }.count }
    private var deletionCount: Int { lines.filter { $0.kind == .deletion }.count }

    private var showCopyButton: Bool {
        horizontalSizeClass == .compact || isHovering
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack {
                Spacer(minLength: 0)
                copyButton
            }

            ScrollView(.horizontal) {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(lines) { line in
                        Text(line.text)
                            .font(Typography.code(12))
                            .foregroundStyle(color(for: line.kind))
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                .fixedSize(horizontal: true, vertical: false)
            }
            .textSelection(.enabled)
        }
        .hoverTracking { hovering in isHovering = hovering }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Diff: \(additionCount) additions, \(deletionCount) deletions")
    }

    private func color(for kind: DiffLineKind) -> Color {
        switch kind {
        case .addition: return .statusSuccess
        case .deletion: return .statusError
        case .hunk, .header, .context: return palette.textSecondary
        }
    }

    private var copyButton: some View {
        Button {
            copyToClipboard(diff)
            didCopy = true
            Task {
                try? await Task.sleep(for: .milliseconds(1500))
                withAnimation(DesignAnimation.standard(reduceMotion)) {
                    didCopy = false
                }
            }
        } label: {
            Image(systemName: didCopy ? "checkmark" : "doc.on.doc")
                .font(Typography.caption())
                .foregroundStyle(didCopy ? Color.statusSuccess : Color.primary.opacity(0.75))
                .frame(
                    minWidth: horizontalSizeClass == .compact ? HitTarget.touch : HitTarget.minimum,
                    minHeight: horizontalSizeClass == .compact ? HitTarget.touch : HitTarget.minimum
                )
        }
        .buttonStyle(.plain)
        .opacity(showCopyButton || didCopy ? 1 : 0)
        .allowsHitTesting(showCopyButton || didCopy)
        .animation(DesignAnimation.standard(reduceMotion), value: showCopyButton)
        .animation(DesignAnimation.standard(reduceMotion), value: didCopy)
        .accessibilityLabel(didCopy ? "Copied" : "Copy diff")
    }
}
