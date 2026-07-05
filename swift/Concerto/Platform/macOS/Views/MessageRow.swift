#if os(macOS)
import SwiftUI
import LoopflowCore

/// One turn in a wave's conversation. A `user` turn is the operator's message
/// (accent bar + selectable prose). An `assistant` turn renders its accumulated
/// prose with fenced-code segmentation, then the command/file/message/thought/
/// tool items it produced, and a blinking cursor while it's still streaming.
///
/// Recovered and rebuilt from the conversations `MessageRow` (git 45ab5d36e^),
/// rebound from the old `SessionMessage`/transcript machinery to the live
/// `ChatTurn` wire model.
struct MessageRow: View {
    @Environment(\.palette) private var palette

    let turn: ChatTurn
    let timestampLabel: String?

    var body: some View {
        if turn.role == .assistant {
            content
                .frame(maxWidth: .infinity, alignment: .leading)
        } else {
            HStack(alignment: .top, spacing: Spacing.sm) {
                accentBar
                content
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            // Attributed emissions (`lf chat` — worker reports, child-wave
            // escalations) carry a speaker byline; plain turns don't.
            if let from = turn.from {
                Text(from)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }

            if turn.role == .assistant {
                assistantBody
            } else {
                Text(turn.text)
                    .font(Typography.body())
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }

            if turn.status == .failed {
                failedBadge
            }

            if let timestampLabel {
                Text(timestampLabel)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
        }
    }

    @ViewBuilder
    private var assistantBody: some View {
        let segments = parseMessageSegments(turn.text)
        VStack(alignment: .leading, spacing: Spacing.sm) {
            ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                switch segment {
                case .text(let text):
                    SelectableAssistantMessageTextView(text: text)
                case .code(let language, let code):
                    CodeBlockView(language: language, content: code)
                }
            }

            if !turn.items.isEmpty {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    ForEach(turn.items) { item in
                        ConversationItemCard(item: item)
                    }
                }
            }

            if turn.status == .running {
                StreamingCursorView()
                    .id("streaming-cursor-\(turn.id)")
            }
        }
    }

    private var accentBar: some View {
        RoundedRectangle(cornerRadius: 1.5)
            .fill(palette.accent)
            .frame(width: 3)
            .frame(minHeight: Spacing.lg)
            .accessibilityHidden(true)
    }

    private var failedBadge: some View {
        HStack(spacing: Spacing.xs) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(Typography.caption(11))
                .foregroundStyle(Color.statusError)
            Text("Turn failed")
                .font(Typography.caption(11))
                .foregroundStyle(Color.statusError)
        }
    }
}

/// A single command/file/message/thought/tool item, with an expandable detail
/// (command output, tool result, or a file diff).
struct ConversationItemCard: View {
    @Environment(\.palette) private var palette

    let item: ConversationItem

    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            header

            if isExpanded {
                detail
            }
        }
        .padding(Spacing.md)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .overlay(
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .stroke(palette.border, lineWidth: 1)
        )
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline, spacing: Spacing.sm) {
            if let status = item.itemStatus {
                Text(status.symbol)
                    .font(Typography.body())
                    .foregroundStyle(status.color)
            }

            Text(item.icon)
                .font(Typography.body())

            Text(item.label)
                .font(item.isThought ? Typography.caption() : Typography.body(item.usesMonospaceLabel ? 13 : 14))
                .fontDesign(item.usesMonospaceLabel ? .monospaced : .default)
                .foregroundStyle(item.isThought ? palette.textSecondary : palette.text)
                .lineLimit(3)
                .frame(maxWidth: .infinity, alignment: .leading)

            trailingBadges

            if hasDetail {
                Button {
                    isExpanded.toggle()
                } label: {
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(isExpanded ? "Collapse detail" : "Expand detail")
            }
        }
    }

    @ViewBuilder
    private var trailingBadges: some View {
        if case let .command(_, _, _, _, _, exitCode, durationMs) = item {
            HStack(spacing: Spacing.xs) {
                if let exitCode, exitCode != 0 {
                    Text("exit \(exitCode)")
                        .font(Typography.caption(10))
                        .foregroundStyle(Color.statusError)
                }
                if let durationMs {
                    Text(formatDuration(durationMs))
                        .font(Typography.caption(10))
                        .foregroundStyle(palette.textSecondary)
                }
            }
        }
    }

    @ViewBuilder
    private var detail: some View {
        switch item {
        case let .file(_, changes, _):
            VStack(alignment: .leading, spacing: Spacing.sm) {
                ForEach(changes.indices, id: \.self) { index in
                    let change = changes[index]
                    VStack(alignment: .leading, spacing: Spacing.xxs) {
                        Text(change.path)
                            .font(Typography.code(12))
                            .foregroundStyle(palette.text)
                        if let diff = change.diff, !diff.isEmpty {
                            DiffLinesView(diff: diff)
                        }
                    }
                }
            }
        case let .command(_, _, cwd, _, output, _, _):
            monospaceDetail(output ?? "", subtitle: cwd.isEmpty ? nil : cwd)
        case let .tool(_, _, _, input, output):
            monospaceDetail([input?.displayString, output].compactMap { $0 }.joined(separator: "\n\n"))
        default:
            EmptyView()
        }
    }

    @ViewBuilder
    private func monospaceDetail(_ text: String, subtitle: String? = nil) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            if let subtitle {
                Text(subtitle)
                    .font(Typography.caption(10))
                    .foregroundStyle(palette.textSecondary)
            }
            HStack {
                Spacer(minLength: 0)
                CopyButton(content: text, isVisible: true)
            }
            ScrollView(.horizontal) {
                Text(text)
                    .font(Typography.code(12))
                    .foregroundStyle(palette.textSecondary)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .fixedSize(horizontal: true, vertical: false)
            }
        }
    }

    private var hasDetail: Bool {
        switch item {
        case let .file(_, changes, _):
            return changes.contains { ($0.diff?.isEmpty == false) } || changes.count > 1
        case let .command(_, _, _, _, output, _, _):
            return output?.isEmpty == false
        case let .tool(_, _, _, input, output):
            return input != nil || output?.isEmpty == false
        default:
            return false
        }
    }

    private func formatDuration(_ ms: Int) -> String {
        if ms < 1000 { return "\(ms)ms" }
        return String(format: "%.1fs", Double(ms) / 1000)
    }
}

private extension ConversationItem {
    var itemStatus: Lifecycle? {
        switch self {
        case let .command(_, _, _, status, _, _, _): return status
        case let .file(_, _, status): return status
        case let .tool(_, _, status, _, _): return status
        case .message, .thought, .unknown: return nil
        }
    }

    var icon: String {
        switch self {
        case .command: return "⌘"
        case .file: return "📄"
        case .message: return "💬"
        case .thought: return "💭"
        case .tool: return "🔧"
        case .unknown: return "•"
        }
    }

    var isThought: Bool {
        if case .thought = self { return true }
        return false
    }

    var usesMonospaceLabel: Bool {
        switch self {
        case .command, .tool: return true
        default: return false
        }
    }

    var label: String {
        switch self {
        case let .command(_, command, _, _, _, _, _):
            return command.joined(separator: " ")
        case let .file(_, changes, _):
            if changes.count == 1 { return changes[0].path }
            return "\(changes.count) files"
        case let .message(_, text, _):
            return text
        case let .thought(_, text):
            return text
        case let .tool(_, name, _, _, _):
            return name
        case let .unknown(_, type):
            return type
        }
    }
}

#endif
