import Foundation
import LoopflowCore

enum ChatRole {
    case user
    case assistant
    case error
    case memory
}

struct ChatMessage: Identifiable, Equatable {
    let id: UUID
    let role: ChatRole
    let content: String
    let timestamp: Date

    init(id: UUID = UUID(), role: ChatRole, content: String, timestamp: Date = Date()) {
        self.id = id
        self.role = role
        self.content = content
        self.timestamp = timestamp
    }
}

struct MemoryStore {
    var blocks: [ChatMemoryBlock] = []

    mutating func setBlocks(_ value: [ChatMemoryBlock]) {
        blocks = value
        sortBlocks()
    }

    mutating func upsert(_ block: ChatMemoryBlock) {
        if let index = blocks.firstIndex(where: { $0.name == block.name }) {
            blocks[index] = block
        } else {
            blocks.append(block)
        }
        sortBlocks()
    }

    mutating func remove(named name: String) {
        blocks.removeAll { $0.name == name }
    }

    private mutating func sortBlocks() {
        blocks.sort {
            if $0.position == $1.position {
                return $0.name < $1.name
            }
            return $0.position < $1.position
        }
    }
}

protocol ChatService: Sendable {
    func listMemoryBlocks(waveId: String) async throws -> [ChatMemoryBlock]
    func upsertMemoryBlock(
        waveId: String,
        name: String,
        content: String,
        position: Int?
    ) async throws -> ChatMemoryBlock
    func deleteMemoryBlock(waveId: String, name: String) async throws
    func startChat(
        waveId: String,
        message: String
    ) async throws
    func streamChatEvents(
        waveId: String
    ) -> AsyncThrowingStream<ChatTurnEvent, Error>
    func listChatMessages(waveId: String) async throws -> [ChatMessageRecord]
}

extension LocalWaveService: ChatService {}

enum ChatTurnState {
    case idle
    case running
    case completed
    case failed
}

@MainActor
@Observable
final class ChatState {
    private static let missingBlockNameMessage = "Memory block name is required."

    let waveId: String

    var messages: [ChatMessage] = []
    var turnState: ChatTurnState = .idle
    var memory = MemoryStore()
    var memoryError: String?

    private var hasLoadedMemory = false
    private var hasLoadedMessages = false
    private let waveService: any ChatService

    init(
        waveId: String,
        waveService: any ChatService = LocalWaveService()
    ) {
        self.waveId = waveId
        self.waveService = waveService
    }

    var isLoading: Bool {
        turnState == .running
    }

    var canSend: Bool {
        turnState != .running
    }

    func loadMemoryIfNeeded() async {
        guard !hasLoadedMemory else { return }
        hasLoadedMemory = await loadMemory()
    }

    func loadMessagesIfNeeded() async {
        guard !hasLoadedMessages else { return }
        do {
            let records = try await waveService.listChatMessages(waveId: waveId)
            if !records.isEmpty {
                messages = records.compactMap(Self.chatMessageFromRecord)
            }
            hasLoadedMessages = true
        } catch {
            // Non-fatal — messages will accumulate from live events.
            hasLoadedMessages = true
        }
    }

    @discardableResult
    func loadMemory() async -> Bool {
        do {
            let blocks = try await waveService.listMemoryBlocks(waveId: waveId)
            memory.setBlocks(blocks)
            memoryError = nil
            return true
        } catch {
            memoryError = error.localizedDescription
            return false
        }
    }

    func send(_ rawText: String) async {
        guard turnState != .running else { return }

        let text = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }

        messages.append(ChatMessage(role: .user, content: text))
        turnState = .running

        do {
            try await waveService.startChat(
                waveId: waveId,
                message: text
            )

            var hasLocalFailure = false
            var terminalState: ChatTurnState?

            for try await event in waveService.streamChatEvents(waveId: waveId) {
                switch event {
                case .message(let content, _):
                    let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
                    if !trimmed.isEmpty {
                        messages.append(ChatMessage(role: .assistant, content: trimmed))
                    }
                case .memoryEdit(let op, let block, let detail):
                    do {
                        let badge = try await applyMemoryEdit(op: op, block: block, detail: detail)
                        messages.append(ChatMessage(role: .memory, content: badge))
                    } catch {
                        memoryError = error.localizedDescription
                        messages.append(ChatMessage(role: .error, content: error.localizedDescription))
                        hasLocalFailure = true
                    }
                case .done:
                    if terminalState == nil {
                        terminalState = .completed
                    }
                case .failed(_, let message):
                    terminalState = .failed
                    let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
                    let display = trimmed.isEmpty ? "Turn failed." : trimmed
                    messages.append(ChatMessage(role: .error, content: display))
                }
            }

            if hasLocalFailure {
                turnState = .failed
                return
            }

            guard let terminalState else {
                messages.append(ChatMessage(role: .error, content: "Turn ended before completion."))
                turnState = .failed
                return
            }
            turnState = terminalState
        } catch {
            messages.append(ChatMessage(role: .error, content: error.localizedDescription))
            turnState = .failed
        }
    }

    func upsertMemoryBlock(name: String, content: String, position: Int?) async {
        guard let trimmedName = validatedMemoryBlockName(name) else { return }

        do {
            let block = try await waveService.upsertMemoryBlock(
                waveId: waveId,
                name: trimmedName,
                content: content,
                position: position
            )
            memory.upsert(block)
            memoryError = nil
        } catch {
            memoryError = error.localizedDescription
        }
    }

    func renameMemoryBlock(
        oldName: String,
        newName: String,
        content: String,
        position: Int
    ) async {
        let trimmedOld = oldName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let trimmedNew = validatedMemoryBlockName(newName) else { return }

        if !trimmedOld.isEmpty && trimmedOld != trimmedNew {
            do {
                try await waveService.deleteMemoryBlock(waveId: waveId, name: trimmedOld)
                memory.remove(named: trimmedOld)
            } catch {
                memoryError = error.localizedDescription
                return
            }
        }

        await upsertMemoryBlock(name: trimmedNew, content: content, position: position)
    }

    func deleteMemoryBlock(name: String) async {
        do {
            try await waveService.deleteMemoryBlock(waveId: waveId, name: name)
            memory.remove(named: name)
            memoryError = nil
        } catch {
            memoryError = error.localizedDescription
        }
    }

    private func applyMemoryEdit(op: String, block: String, detail: String) async throws -> String {
        let normalizedBlock = block.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedBlock.isEmpty else {
            throw WaveServiceError.commandFailed("Agent memory edit missing block name.")
        }

        if op.lowercased() == "delete" {
            try await waveService.deleteMemoryBlock(waveId: waveId, name: normalizedBlock)
            memory.remove(named: normalizedBlock)
            memoryError = nil
            return "Agent updated memory: deleted \(normalizedBlock)"
        }

        let existingPosition = memory.blocks.first(where: { $0.name == normalizedBlock })?.position
        let updated = try await waveService.upsertMemoryBlock(
            waveId: waveId,
            name: normalizedBlock,
            content: detail,
            position: existingPosition
        )
        memory.upsert(updated)
        memoryError = nil
        return "Agent updated memory: \(normalizedBlock)"
    }

    private func validatedMemoryBlockName(_ raw: String) -> String? {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            memoryError = Self.missingBlockNameMessage
            return nil
        }
        return trimmed
    }

    private static func chatMessageFromRecord(_ record: ChatMessageRecord) -> ChatMessage? {
        let role: ChatRole
        switch record.role {
        case "user": role = .user
        case "assistant": role = .assistant
        case "memory": role = .memory
        case "error": role = .error
        default: return nil
        }
        return ChatMessage(
            role: role,
            content: record.content,
            timestamp: record.createdAt ?? Date()
        )
    }
}
