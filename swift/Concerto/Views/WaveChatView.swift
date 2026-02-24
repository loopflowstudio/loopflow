import SwiftUI
import LoopflowCore

struct WaveChatView: View {
    @Environment(\.palette) private var palette

    @Bindable var state: ChatState

    @State private var composerText = ""

    var body: some View {
        VStack(spacing: Spacing.md) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.md) {
                        if state.messages.isEmpty {
                            Text("Start an interactive session.")
                                .font(Typography.body())
                                .foregroundStyle(palette.textSecondary)
                                .padding(.vertical, Spacing.xl)
                        }

                        ForEach(state.messages) { message in
                            ChatBubble(message: message)
                                .id(message.id)
                        }

                        if state.isLoading {
                            ProgressView("Thinking…")
                                .font(Typography.caption())
                                .foregroundStyle(palette.textSecondary)
                                .padding(.top, Spacing.sm)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(Spacing.lg)
                }
                .background(palette.background)
                .onChange(of: state.messages.count) { _, _ in
                    if let last = state.messages.last {
                        withAnimation {
                            proxy.scrollTo(last.id, anchor: .bottom)
                        }
                    }
                }
            }

            HStack(alignment: .bottom, spacing: Spacing.sm) {
                TextField("Message", text: $composerText, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...6)
                    .disabled(!state.canSend)

                Button("Send") {
                    sendMessage()
                }
                .keyboardShortcut(.return, modifiers: .command)
                .buttonStyle(DarkButtonStyle())
                .disabled(composerText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !state.canSend)
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.bottom, Spacing.lg)
        }
        .background(palette.background)
    }

    private func sendMessage() {
        let text = composerText
        composerText = ""
        Task {
            await state.send(text)
        }
    }
}

private struct ChatBubble: View {
    @Environment(\.palette) private var palette
    let message: ChatMessage

    var body: some View {
        HStack {
            if message.role == .user { Spacer(minLength: 48) }

            VStack(alignment: .leading, spacing: Spacing.xs) {
                content
                Text(message.timestamp, style: .time)
                    .font(Typography.caption(11))
                    .foregroundStyle(palette.textSecondary)
            }
            .padding(Spacing.md)
            .background(backgroundColor)
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
            .frame(maxWidth: 620, alignment: .leading)

            if message.role != .user { Spacer(minLength: 48) }
        }
    }

    @ViewBuilder
    private var content: some View {
        if message.role == .assistant,
           let markdown = try? AttributedString(
               markdown: message.content,
               options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
           ) {
            Text(markdown)
                .font(Typography.body())
                .foregroundStyle(palette.text)
        } else {
            Text(message.content)
                .font(Typography.body())
                .foregroundStyle(message.role == .error ? .statusError : palette.text)
        }
    }

    private var backgroundColor: Color {
        switch message.role {
        case .user:
            return palette.accent.opacity(0.12)
        case .assistant:
            return palette.surface
        case .error:
            return Color.statusError.opacity(0.12)
        }
    }
}
