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

    mutating func upsert(_ block: ChatMemoryBlock) {
        if let index = blocks.firstIndex(where: { $0.name == block.name }) {
            blocks[index] = block
        } else {
            blocks.append(block)
        }
        blocks.sort { $0.position < $1.position }
    }

    mutating func remove(named name: String) {
        blocks.removeAll { $0.name == name }
    }

    func systemPrompt() -> String {
        guard !blocks.isEmpty else { return "" }
        var lines: [String] = ["<memory>"]
        for block in blocks {
            lines.append("<block name=\"\(xmlEscape(block.name))\">")
            lines.append(xmlEscape(block.content))
            lines.append("</block>")
        }
        lines.append("</memory>")
        return lines.joined(separator: "\n")
    }

    private func xmlEscape(_ value: String) -> String {
        value
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "'", with: "&apos;")
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
    func createChatTurn(
        waveId: String,
        message: String,
        memoryBlocks: [ChatMemoryBlock]
    ) async throws -> ChatTurn
    func streamChatTurnEvents(
        waveId: String,
        turnId: String
    ) -> AsyncThrowingStream<ChatTurnEvent, Error>
}

extension LocalWaveService: ChatService {}

enum ChatTurnState {
    case idle
    case running
    case completed
    case failed
}

private enum ChatStateError: LocalizedError {
    case invalidMemoryEdit(String)

    var errorDescription: String? {
        switch self {
        case .invalidMemoryEdit(let message):
            return message
        }
    }
}

@MainActor
@Observable
final class ChatState {
    private static let missingBlockNameMessage = "Memory block name is required."
    private static let invalidCompletionContractMessage = "Turn failed: expected exactly one final message."

    let waveId: String

    var messages: [ChatMessage] = []
    var turnState: ChatTurnState = .idle
    var memory = MemoryStore()
    var memoryError: String?

    private var hasLoadedMemory = false
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
            let turn = try await waveService.createChatTurn(
                waveId: waveId,
                message: text,
                memoryBlocks: memory.blocks
            )

            var finalMessageCount = 0
            var sawFailure = false
            var sawTerminal = false

            for try await event in waveService.streamChatTurnEvents(waveId: waveId, turnId: turn.id) {
                switch event {
                case .message(let content, let phase):
                    let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
                    if !trimmed.isEmpty {
                        messages.append(ChatMessage(role: .assistant, content: trimmed))
                    }
                    if phase == .final {
                        finalMessageCount += 1
                    }
                case .memoryEdit(let op, let block, let detail):
                    do {
                        let badge = try await applyMemoryEdit(op: op, block: block, detail: detail)
                        messages.append(ChatMessage(role: .memory, content: badge))
                    } catch {
                        memoryError = error.localizedDescription
                        messages.append(ChatMessage(role: .error, content: error.localizedDescription))
                        sawFailure = true
                    }
                case .done:
                    sawTerminal = true
                case .failed(_, let message):
                    sawFailure = true
                    sawTerminal = true
                    let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
                    let display = trimmed.isEmpty ? "Turn failed." : trimmed
                    messages.append(ChatMessage(role: .error, content: display))
                }
            }

            if sawFailure {
                turnState = .failed
                return
            }

            guard sawTerminal else {
                messages.append(ChatMessage(role: .error, content: "Turn ended before completion."))
                turnState = .failed
                return
            }

            guard finalMessageCount == 1 else {
                messages.append(ChatMessage(role: .error, content: Self.invalidCompletionContractMessage))
                turnState = .failed
                return
            }

            turnState = .completed
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
            throw ChatStateError.invalidMemoryEdit("Agent memory edit missing block name.")
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
}
