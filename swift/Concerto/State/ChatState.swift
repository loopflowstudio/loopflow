import Foundation
import LoopflowCore

enum ChatRole {
    case user
    case assistant
    case error
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

protocol ChatMemoryService: Sendable {
    func listMemoryBlocks(waveId: String) async throws -> [ChatMemoryBlock]
    func upsertMemoryBlock(
        waveId: String,
        name: String,
        content: String,
        position: Int?
    ) async throws -> ChatMemoryBlock
    func deleteMemoryBlock(waveId: String, name: String) async throws
}

extension LocalWaveService: ChatMemoryService {}

@MainActor
@Observable
final class ChatState {
    private static let missingAPIKeyMessage = "Set ANTHROPIC_API_KEY to enable chat."
    private static let missingBlockNameMessage = "Memory block name is required."

    let waveId: String

    var messages: [ChatMessage] = []
    var isLoading = false
    var memory = MemoryStore()
    var memoryError: String?

    private var hasLoadedMemory = false

    private let waveService: any ChatMemoryService
    private let anthropic: AnthropicClient

    init(
        waveId: String,
        waveService: any ChatMemoryService = LocalWaveService(),
        anthropic: AnthropicClient = AnthropicClient()
    ) {
        self.waveId = waveId
        self.waveService = waveService
        self.anthropic = anthropic
    }

    var canSend: Bool {
        !isLoading && anthropic.hasAPIKey
    }

    var missingAPIKey: Bool {
        !anthropic.hasAPIKey
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
        let text = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }

        messages.append(ChatMessage(role: .user, content: text))

        guard anthropic.hasAPIKey else {
            messages.append(ChatMessage(role: .error, content: Self.missingAPIKeyMessage))
            return
        }

        isLoading = true
        defer { isLoading = false }

        do {
            let reply = try await anthropic.complete(
                message: text,
                system: memory.systemPrompt()
            )
            messages.append(ChatMessage(role: .assistant, content: reply))
        } catch {
            messages.append(ChatMessage(role: .error, content: error.localizedDescription))
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

    private func validatedMemoryBlockName(_ raw: String) -> String? {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            memoryError = Self.missingBlockNameMessage
            return nil
        }
        return trimmed
    }
}
