#if os(macOS)
import SwiftUI
import Loopflow

// Shared rendering pieces for WaveChat turns: fenced-code segmentation, the code
// block + diff views, the streaming cursor, and a hover-reveal copy button.
// Recovered from the conversations UI (git 45ab5d36e^) and rebound to the
// live wave chat models.

// MARK: - Fenced-code segmentation

enum MessageSegment: Equatable {
    case text(String)
    case code(language: String?, content: String)
}

func parseMessageSegments(_ content: String) -> [MessageSegment] {
    guard !content.isEmpty else { return [] }
    guard content.contains("```") else { return [.text(content)] }

    let lines = content.split(omittingEmptySubsequences: false, whereSeparator: \.isNewline)
    var segments: [MessageSegment] = []
    var textLines: [String] = []
    var codeLines: [String] = []
    var currentLanguage: String?
    var inCodeBlock = false

    func flushText() {
        let value = textLines.joined(separator: "\n")
        if !value.isEmpty {
            segments.append(.text(value))
        }
        textLines.removeAll(keepingCapacity: true)
    }

    func flushCode() {
        let value = codeLines.joined(separator: "\n")
        segments.append(.code(language: currentLanguage, content: value))
        codeLines.removeAll(keepingCapacity: true)
        currentLanguage = nil
    }

    for rawLine in lines {
        let line = String(rawLine)
        let trimmed = line.trimmingCharacters(in: .whitespaces)

        if inCodeBlock {
            if trimmed == "```" {
                flushCode()
                inCodeBlock = false
            } else {
                codeLines.append(line)
            }
            continue
        }

        if trimmed.hasPrefix("```") {
            flushText()
            inCodeBlock = true
            let suffix = trimmed.dropFirst(3).trimmingCharacters(in: .whitespaces)
            currentLanguage = suffix.isEmpty ? nil : String(suffix)
        } else {
            textLines.append(line)
        }
    }

    if inCodeBlock {
        flushCode()
    } else {
        flushText()
    }

    return segments.isEmpty ? [.text(content)] : segments
}

// MARK: - Streaming cursor

struct StreamingCursorView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isVisible = false

    var body: some View {
        Text("▊")
            .font(Typography.code(13))
            .foregroundStyle(Color.statusInfo)
            .opacity(reduceMotion ? 1 : (isVisible ? 1 : 0))
            .onAppear {
                guard !reduceMotion else {
                    isVisible = true
                    return
                }
                isVisible = false
                withAnimation(.easeInOut(duration: 0.6).repeatForever(autoreverses: true)) {
                    isVisible = true
                }
            }
            .accessibilityLabel("Streaming")
    }
}

// MARK: - Copy button

struct CopyButton: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let content: String
    let isVisible: Bool

    @State private var didCopy = false

    private var isInteractive: Bool { isVisible || didCopy }

    var body: some View {
        Button {
            copyToClipboard(content)
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
                .frame(minWidth: HitTarget.minimum, minHeight: HitTarget.minimum)
        }
        .buttonStyle(.plain)
        .opacity(isInteractive ? 1 : 0)
        .allowsHitTesting(isInteractive)
        .animation(DesignAnimation.standard(reduceMotion), value: isVisible)
        .animation(DesignAnimation.standard(reduceMotion), value: didCopy)
        .accessibilityLabel(didCopy ? "Copied" : "Copy")
    }
}

// MARK: - Code block

struct CodeBlockView: View {
    @Environment(\.palette) private var palette

    let language: String?
    let content: String

    @State private var isHovering = false

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                if let language, !language.isEmpty {
                    Text(language.uppercased())
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                        .padding(.horizontal, Spacing.sm)
                        .padding(.vertical, Spacing.xxs)
                        .background(palette.surfaceMuted)
                        .clipShape(Capsule())
                        .accessibilityLabel("Code language \(language)")
                }

                Spacer(minLength: 0)

                CopyButton(content: content, isVisible: isHovering)
            }

            ScrollView(.horizontal) {
                Text(content)
                    .font(Typography.code(13))
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: true, vertical: false)
            }
        }
        .padding(Spacing.md)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .overlay(
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .stroke(palette.border, lineWidth: 1)
        )
        .hoverTracking { isHovering = $0 }
    }
}

// MARK: - Diff

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

struct DiffLinesView: View {
    @Environment(\.palette) private var palette
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let diff: String

    @State private var isHovering = false

    private var lines: [DiffLine] { parseDiffLines(diff) }
    private var additionCount: Int { lines.filter { $0.kind == .addition }.count }
    private var deletionCount: Int { lines.filter { $0.kind == .deletion }.count }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack {
                Spacer(minLength: 0)
                CopyButton(content: diff, isVisible: isHovering)
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
        .hoverTracking { isHovering = $0 }
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
}

// MARK: - Item status glyphs

extension Lifecycle {
    var symbol: String {
        switch self {
        case .pending: return "…"
        case .running: return "▶"
        case .completed: return "✓"
        case .failed: return "✗"
        case .interrupted: return "⊘"
        }
    }

    var color: Color {
        switch self {
        case .pending, .running: return .statusInfo
        case .completed: return .statusSuccess
        case .failed: return .statusError
        case .interrupted: return .statusWarning
        }
    }
}

#endif
