import SwiftUI
import LoopflowCore

// MARK: - Reply Demo View

private enum ReplyDemoInteractionStyle: String, CaseIterable, Identifiable {
    case popover = "Option A · Popover"
    case inline = "Option B · Inline"

    var id: String { rawValue }
}

private enum ReplyDemoEmojiVariant: String, CaseIterable, Identifiable {
    case fixed = "Fixed palette"
    case picker = "Emoji picker"

    var id: String { rawValue }
}

private enum ReplyDemoTrayState: String, CaseIterable, Identifiable {
    case empty = "Empty"
    case one = "1 item"
    case three = "3 items"
    case mixed = "Full mixed"

    var id: String { rawValue }

    var queue: ReplyQueue {
        switch self {
        case .empty: return .demoQueueEmpty()
        case .one: return .demoQueueSingle()
        case .three: return .demoQueueTriple()
        case .mixed: return .demoQueueFullMixed()
        }
    }
}

struct ReplyDemoView: View {

    @Environment(\.palette) private var palette

    @State private var interactionStyle: ReplyDemoInteractionStyle = .popover
    @State private var emojiVariant: ReplyDemoEmojiVariant = .fixed
    @State private var trayState: ReplyDemoTrayState = .three
    @State private var queue: ReplyQueue = .demoQueueTriple()
    @State private var trayExpanded = true

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Spacing.lg) {
                Text("Reply Demo")
                    .font(Typography.sectionTitle(28))

                Text("Prototype quote-reply interactions and assembled output before wiring into live sessions.")
                    .font(Typography.body())
                    .foregroundStyle(palette.textSecondary)

                controls

                HStack(alignment: .top, spacing: Spacing.lg) {
                    ReplyInteractionOptionCard(
                        style: interactionStyle,
                        emojiVariant: emojiVariant
                    )

                    VStack(alignment: .leading, spacing: Spacing.md) {
                        ReplyDraftTray(
                            queue: queue,
                            isExpanded: $trayExpanded
                        )

                        assembledPreview
                    }
                    .frame(maxWidth: .infinity)
                }
            }
            .padding(Spacing.lg)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(palette.background)
        .onChange(of: trayState) { _, newValue in
            queue = newValue.queue
            trayExpanded = true
        }
    }

    private var controls: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Picker("Interaction", selection: $interactionStyle) {
                ForEach(ReplyDemoInteractionStyle.allCases) { style in
                    Text(style.rawValue).tag(style)
                }
            }
            .pickerStyle(.segmented)

            HStack(spacing: Spacing.md) {
                Picker("Emoji", selection: $emojiVariant) {
                    ForEach(ReplyDemoEmojiVariant.allCases) { variant in
                        Text(variant.rawValue).tag(variant)
                    }
                }
                .pickerStyle(.segmented)

                Picker("Tray state", selection: $trayState) {
                    ForEach(ReplyDemoTrayState.allCases) { state in
                        Text(state.rawValue).tag(state)
                    }
                }
                .pickerStyle(.segmented)
            }
        }
    }

    private var assembledPreview: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text("Assembled message preview")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            ScrollView {
                Text(queue.assembleMessage())
                    .font(Typography.code(12))
                    .foregroundStyle(palette.text)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
                    .padding(Spacing.md)
            }
            .frame(minHeight: 220)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .fill(palette.surfaceMuted)
            )
        }
    }
}

private struct ReplyInteractionOptionCard: View {
    let style: ReplyDemoInteractionStyle
    let emojiVariant: ReplyDemoEmojiVariant

    @Environment(\.palette) private var palette

    private let highlightedQuote = "use a UUID for the junction table"

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("Mock LLM response")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            VStack(alignment: .leading, spacing: Spacing.sm) {
                Text("I recommend three changes before shipping:")
                Text("• Use a UUID for the junction table")
                    .padding(.horizontal, Spacing.xs)
                    .background(
                        RoundedRectangle(cornerRadius: CornerRadius.sm)
                            .fill(palette.accent.opacity(0.18))
                    )
                Text("• Add a migration script")
                Text("• Convert test fixtures to shared helpers")

                Text("```swift\nstruct JoinKey {\n  let id: UUID\n}\n```")
                    .font(Typography.code())
                    .padding(Spacing.sm)
                    .background(
                        RoundedRectangle(cornerRadius: CornerRadius.sm)
                            .fill(palette.surfaceMuted)
                    )
            }
            .font(Typography.body())
            .foregroundStyle(palette.text)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Spacing.md)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .fill(palette.surface)
            )
            .overlay(alignment: .topLeading) {
                if style == .popover {
                    floatingToolbar
                        .offset(x: 80, y: -18)
                }
            }

            if style == .inline {
                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text("> \(highlightedQuote)")
                        .font(Typography.caption())
                        .foregroundStyle(palette.textSecondary)
                    Text("No — UUID makes downstream joins less painful.")
                        .font(Typography.body())
                }
                .padding(Spacing.sm)
                .background(
                    RoundedRectangle(cornerRadius: CornerRadius.sm)
                        .fill(palette.surfaceMuted)
                )
            }
        }
        .frame(maxWidth: .infinity)
    }

    private var floatingToolbar: some View {
        HStack(spacing: Spacing.xs) {
            Text("Reply")
                .font(Typography.caption())
                .padding(.horizontal, Spacing.xs)
                .padding(.vertical, Spacing.xxs)
                .background(
                    RoundedRectangle(cornerRadius: CornerRadius.sm)
                        .fill(palette.background)
                )

            if emojiVariant == .fixed {
                ForEach(["👍", "👎", "✏️", "❓"], id: \.self) { emoji in
                    Text(emoji)
                }
            } else {
                Text("🙂 ▾")
                    .font(Typography.caption())
            }
        }
        .padding(.horizontal, Spacing.xs)
        .padding(.vertical, Spacing.xxs)
        .background(
            RoundedRectangle(cornerRadius: CornerRadius.sm)
                .fill(palette.surface)
                .shadow(color: .black.opacity(0.12), radius: 6, y: 2)
        )
    }
}

#Preview {
    ReplyDemoView()
        .frame(width: 1220, height: 760)
}
