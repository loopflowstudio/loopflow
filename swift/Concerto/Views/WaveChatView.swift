import SwiftUI
import LoopflowCore

struct WaveChatView: View {
    @Environment(\.palette) private var palette

    @Bindable var state: ChatState

    @State private var composerText = ""
    @State private var newBlockName = ""
    @State private var newBlockContent = ""

    var body: some View {
        HSplitView {
            chatThread
                .frame(minWidth: 520)

            memoryPanel
                .frame(minWidth: 320, maxWidth: 420)
        }
        .task {
            await state.loadMemoryIfNeeded()
        }
    }

    private var chatThread: some View {
        VStack(spacing: Spacing.md) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: Spacing.md) {
                        if state.messages.isEmpty {
                            Text("Start a chat. Only memory blocks are sent as context between turns.")
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

    private var memoryPanel: some View {
        VStack(alignment: .leading, spacing: Spacing.md) {
            Text("Memory")
                .font(Typography.sectionTitle(18))

            Text("Editable context sent on every request.")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            VStack(alignment: .leading, spacing: Spacing.sm) {
                TextField("Block name", text: $newBlockName)
                    .textFieldStyle(.roundedBorder)
                TextEditor(text: $newBlockContent)
                    .font(Typography.body())
                    .frame(minHeight: 64)
                    .padding(Spacing.xs)
                    .background(palette.surfaceMuted)
                    .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))

                Button("Add Memory Block") {
                    Task {
                        await state.upsertMemoryBlock(
                            name: newBlockName,
                            content: newBlockContent,
                            position: nil
                        )
                        if state.memoryError == nil {
                            newBlockName = ""
                            newBlockContent = ""
                        }
                    }
                }
                .buttonStyle(GhostButtonStyle())
                .disabled(newBlockName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }

            if let memoryError = state.memoryError {
                Text(memoryError)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
            }

            ScrollView {
                LazyVStack(spacing: Spacing.sm) {
                    ForEach(state.memory.blocks) { block in
                        MemoryBlockEditor(
                            block: block,
                            onSave: { name, content in
                                Task {
                                    await state.renameMemoryBlock(
                                        oldName: block.name,
                                        newName: name,
                                        content: content,
                                        position: block.position
                                    )
                                }
                            },
                            onDelete: {
                                Task { await state.deleteMemoryBlock(name: block.name) }
                            }
                        )
                    }
                }
                .padding(.bottom, Spacing.sm)
            }
        }
        .padding(Spacing.lg)
        .background(palette.surface)
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
        if message.role == .memory {
            Label(message.content, systemImage: "brain")
                .font(Typography.caption())
                .foregroundStyle(Color.statusInfo)
        } else if message.role == .assistant,
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
        case .memory:
            return Color.statusInfo.opacity(0.12)
        }
    }
}

private struct MemoryBlockEditor: View {
    @Environment(\.palette) private var palette

    let block: ChatMemoryBlock
    let onSave: (String, String) -> Void
    let onDelete: () -> Void

    @State private var name: String
    @State private var content: String

    init(
        block: ChatMemoryBlock,
        onSave: @escaping (String, String) -> Void,
        onDelete: @escaping () -> Void
    ) {
        self.block = block
        self.onSave = onSave
        self.onDelete = onDelete
        _name = State(initialValue: block.name)
        _content = State(initialValue: block.content)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            TextField("Name", text: $name)
                .textFieldStyle(.roundedBorder)

            TextEditor(text: $content)
                .font(Typography.body())
                .frame(minHeight: 80)
                .padding(Spacing.xs)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))

            HStack(spacing: Spacing.sm) {
                Button("Save") {
                    onSave(name, content)
                }
                .buttonStyle(GhostButtonStyle())

                Button("Delete", role: .destructive) {
                    onDelete()
                }
                .buttonStyle(.borderless)
            }
        }
        .padding(Spacing.sm)
        .background(palette.background)
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
        .overlay(
            RoundedRectangle(cornerRadius: CornerRadius.md)
                .stroke(palette.border, lineWidth: 1)
        )
    }
}
