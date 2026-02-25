import Foundation
import LoopflowCore

enum ChatRole {
    case user
    case assistant
    case error
    case system
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

enum SessionItemType: Equatable {
    case command
    case file
    case message
    case thought
    case tool
    case unknown
}

struct TranscriptItemCard: Equatable {
    let type: SessionItemType
    let label: String
    let status: ItemStatus?
    let detail: String?
}

struct TranscriptItem: Identifiable, Equatable {
    let id: UUID
    let turnId: String
    let itemId: String
    let card: TranscriptItemCard
    let timestamp: Date

    init(
        id: UUID = UUID(),
        turnId: String,
        itemId: String,
        card: TranscriptItemCard,
        timestamp: Date = Date()
    ) {
        self.id = id
        self.turnId = turnId
        self.itemId = itemId
        self.card = card
        self.timestamp = timestamp
    }
}

enum TranscriptEntry: Identifiable, Equatable {
    case message(ChatMessage)
    case item(TranscriptItem)

    var id: UUID {
        switch self {
        case .message(let message):
            return message.id
        case .item(let item):
            return item.id
        }
    }

    var timestamp: Date {
        switch self {
        case .message(let message):
            return message.timestamp
        case .item(let item):
            return item.timestamp
        }
    }
}

protocol ChatService: Sendable {
    func createSession(
        provider: String,
        waveRunId: String?,
        config: AgentSessionConfig
    ) async throws -> AgentSession
    func getSession(_ id: String) async throws -> AgentSession
    func sendSessionInput(sessionId: String, content: String) async throws -> AgentSession
    func streamSessionEvents(
        sessionId: String,
        afterSeq: Int?
    ) -> AsyncThrowingStream<AgentSessionEventEnvelope, Error>
    func stopSession(_ id: String) async throws -> AgentSession
}

extension LocalWaveService: ChatService {}

enum ChatTurnState {
    case idle
    case running
    case completed
    case failed
}

enum StreamPhase {
    case idle
    case replaying
    case live
    case ending
}

@MainActor
@Observable
final class ChatState {
    let waveId: String

    var transcript: [TranscriptEntry] = []
    var turnState: ChatTurnState = .idle
    var streamPhase: StreamPhase = .idle

    private(set) var itemsById: [String: SessionItem] = [:]

    private let sessionProvider: String
    private let sessionWaveRunId: String?
    private let sessionConfig: AgentSessionConfig
    private let waveService: any ChatService
    private let userDefaults: UserDefaults
    private let detailLimit = 16_000
    private let truncationSuffix = "…truncated"

    private var sessionId: String?
    private var lastAppliedSeq: Int?
    private var currentTurnId: String?

    private var itemEntryIdByItemId: [String: UUID] = [:]
    private var assistantEntryIdByTurnId: [String: UUID] = [:]

    private var streamTask: Task<Void, Never>?
    private var streamGeneration = 0

    init(
        waveId: String,
        sessionProvider: String = "claude",
        sessionWaveRunId: String? = nil,
        sessionConfig: AgentSessionConfig,
        waveService: any ChatService = LocalWaveService(),
        userDefaults: UserDefaults = .standard
    ) {
        self.waveId = waveId
        self.sessionProvider = sessionProvider
        self.sessionWaveRunId = sessionWaveRunId
        self.sessionConfig = sessionConfig
        self.waveService = waveService
        self.userDefaults = userDefaults
    }

    var isLoading: Bool {
        turnState == .running || streamPhase == .replaying || streamPhase == .ending
    }

    var canSend: Bool {
        turnState != .running && streamPhase != .replaying && streamPhase != .ending
    }

    var canEndSession: Bool {
        sessionId != nil && streamPhase != .ending
    }

    func onAppear() async {
        if let sessionId {
            if streamTask == nil {
                startStream(sessionId: sessionId, afterSeq: lastAppliedSeq, phase: .live)
            }
            return
        }

        await reconnectIfNeeded()
    }

    func onDisappear() {
        cancelStreamTask()
        if streamPhase != .ending {
            streamPhase = .idle
        }
    }

    func send(_ rawText: String) async {
        let text = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }

        guard streamPhase != .replaying else {
            appendMessage(role: .system, content: "Replaying… Please wait.")
            return
        }

        guard turnState != .running && streamPhase != .ending else { return }

        appendMessage(role: .user, content: text)
        turnState = .running

        do {
            let sessionId = try await ensureSession()
            if streamTask == nil {
                startStream(sessionId: sessionId, afterSeq: lastAppliedSeq, phase: .live)
            }
            _ = try await waveService.sendSessionInput(sessionId: sessionId, content: text)
        } catch {
            appendMessage(role: .error, content: error.localizedDescription)
            turnState = .failed
        }
    }

    func endSession() async {
        guard let sessionId else { return }

        streamPhase = .ending
        cancelStreamTask()

        do {
            _ = try await waveService.stopSession(sessionId)
        } catch {
            appendMessage(role: .error, content: error.localizedDescription)
        }

        self.sessionId = nil
        persistSessionId(nil)
        currentTurnId = nil
        turnState = .idle
        streamPhase = .idle
        appendMessage(role: .system, content: "Session ended")
    }

    func reconnectIfNeeded() async {
        guard sessionId == nil else { return }
        guard let storedSessionId = userDefaults.string(forKey: sessionIdKey) else { return }

        streamPhase = .replaying

        do {
            let session = try await waveService.getSession(storedSessionId)
            guard session.status == "active" else {
                persistSessionId(nil)
                streamPhase = .idle
                return
            }

            sessionId = session.id
            resetForReplay()
            appendMessage(role: .system, content: "Replaying…")
            startStream(sessionId: storedSessionId, afterSeq: nil, phase: .replaying)
        } catch {
            persistSessionId(nil)
            streamPhase = .idle
        }
    }

    private var sessionIdKey: String {
        "chatSession.\(waveId)"
    }

    private func persistSessionId(_ id: String?) {
        if let id {
            userDefaults.set(id, forKey: sessionIdKey)
        } else {
            userDefaults.removeObject(forKey: sessionIdKey)
        }
    }

    private func ensureSession() async throws -> String {
        if let existing = sessionId {
            return existing
        }

        resetSessionCaches()
        let session = try await waveService.createSession(
            provider: sessionProvider,
            waveRunId: sessionWaveRunId,
            config: sessionConfig
        )
        sessionId = session.id
        persistSessionId(session.id)
        appendMessage(role: .system, content: "Session started")

        if !isSessionActiveStatus(session.status) {
            try await waitForActiveSession(sessionId: session.id)
        }
        return session.id
    }

    private func waitForActiveSession(sessionId: String) async throws {
        var lastStatus: String = "starting"
        for attempt in 0..<20 {
            let session = try await waveService.getSession(sessionId)
            lastStatus = session.status

            if isSessionActiveStatus(session.status) {
                return
            }

            if isSessionTerminalStatus(session.status) {
                throw NSError(
                    domain: "ChatState",
                    code: 1,
                    userInfo: [NSLocalizedDescriptionKey: "Session ended before becoming active (\(session.status))"]
                )
            }

            if attempt < 19 {
                try await Task.sleep(for: .milliseconds(150))
            }
        }

        throw NSError(
            domain: "ChatState",
            code: 2,
            userInfo: [NSLocalizedDescriptionKey: "Session is still \(lastStatus). Please retry in a moment."]
        )
    }

    private func isSessionActiveStatus(_ status: String) -> Bool {
        status.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "active"
    }

    private func isSessionTerminalStatus(_ status: String) -> Bool {
        let normalized = status.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return normalized == "ended" || normalized == "failed"
    }

    private func startStream(sessionId: String, afterSeq: Int?, phase: StreamPhase) {
        cancelStreamTask()
        streamGeneration += 1
        let generation = streamGeneration
        streamPhase = phase

        streamTask = Task { [weak self] in
            guard let self else { return }
            await self.consumeStream(
                sessionId: sessionId,
                afterSeq: afterSeq,
                generation: generation,
                reconnecting: phase == .replaying
            )
        }
    }

    private func consumeStream(
        sessionId: String,
        afterSeq: Int?,
        generation: Int,
        reconnecting: Bool
    ) async {
        do {
            for try await envelope in waveService.streamSessionEvents(sessionId: sessionId, afterSeq: afterSeq) {
                if envelope.replayCompletedLastSeq != nil {
                    promoteToLiveIfCurrent(generation: generation)
                    continue
                }
                applyEnvelope(envelope)
            }

            if reconnecting {
                promoteToLiveIfCurrent(generation: generation)
            }
        } catch is CancellationError {
            return
        } catch {
            guard generation == streamGeneration else { return }
            appendMessage(role: .error, content: error.localizedDescription)
            turnState = .failed
            if streamPhase != .ending {
                streamPhase = .idle
            }
            streamTask = nil
            return
        }

        guard generation == streamGeneration else { return }
        streamTask = nil
        if streamPhase != .ending {
            streamPhase = .idle
        }
    }

    private func promoteToLiveIfCurrent(generation: Int) {
        guard generation == streamGeneration else { return }
        if streamPhase == .replaying {
            streamPhase = .live
        }
    }

    private func cancelStreamTask() {
        streamTask?.cancel()
        streamTask = nil
    }

    private func resetForReplay() {
        transcript.removeAll()
        turnState = .idle
        resetSessionCaches()
    }

    private func resetSessionCaches() {
        currentTurnId = nil
        lastAppliedSeq = nil
        itemsById.removeAll()
        itemEntryIdByItemId.removeAll()
        assistantEntryIdByTurnId.removeAll()
    }

    private func appendMessage(role: ChatRole, content: String) {
        transcript.append(.message(ChatMessage(role: role, content: content)))
    }

    private func applyEnvelope(_ envelope: AgentSessionEventEnvelope) {
        guard let event = envelope.event else { return }

        if let seq = envelope.seq {
            if let lastAppliedSeq, seq <= lastAppliedSeq {
                return
            }
            lastAppliedSeq = seq
        }

        reduce(event)
    }

    private func reduce(_ event: AgentSessionEvent) {
        switch event {
        case .turnStarted(let turnId):
            currentTurnId = turnId
            turnState = .running

        case .turnCompleted(let turnId, let status):
            guard currentTurnId == nil || currentTurnId == turnId else { return }
            currentTurnId = nil
            if status == "completed" {
                turnState = .completed
            } else {
                turnState = .failed
            }

        case .textDelta(let turnId, let content):
            guard !content.isEmpty else { return }
            appendAssistantDelta(turnId: turnId, delta: content)

        case .itemStarted(let turnId, let item):
            upsertItem(turnId: turnId, item: item)

        case .itemUpdated(let turnId, let itemId, let delta):
            applyItemUpdate(turnId: turnId, itemId: itemId, delta: delta)

        case .itemCompleted(let turnId, let item):
            upsertItem(turnId: turnId, item: item)

        case .diffUpdated(_, _):
            return

        case .statusChanged(let status):
            if status == "ended" || status == "failed" {
                streamPhase = .idle
            }

        case .error(_, let message):
            let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
            if !trimmed.isEmpty {
                appendMessage(role: .error, content: trimmed)
            }
            turnState = .failed

        case .reasoningDelta, .other:
            return
        }
    }

    private func appendAssistantDelta(turnId: String, delta: String) {
        if let entryId = assistantEntryIdByTurnId[turnId],
           let index = transcript.firstIndex(where: { $0.id == entryId }),
           case .message(let message) = transcript[index],
           message.role == .assistant {
            let updated = ChatMessage(
                id: message.id,
                role: .assistant,
                content: message.content + delta,
                timestamp: message.timestamp
            )
            transcript[index] = .message(updated)
            return
        }

        let message = ChatMessage(role: .assistant, content: delta)
        transcript.append(.message(message))
        assistantEntryIdByTurnId[turnId] = message.id
    }

    private func upsertItem(turnId: String, item: SessionItem) {
        guard let itemId = itemIdentifier(item) else { return }
        let boundedItem = boundedSessionItem(item)
        itemsById[itemId] = boundedItem

        if itemEntryIdByItemId[itemId] == nil {
            let entry = TranscriptItem(
                turnId: turnId,
                itemId: itemId,
                card: projectCard(from: boundedItem)
            )
            itemEntryIdByItemId[itemId] = entry.id
            transcript.append(.item(entry))
            return
        }

        updateTranscriptItem(itemId: itemId, turnId: turnId, item: boundedItem)
    }

    private func applyItemUpdate(turnId: String, itemId: String, delta: ItemDelta) {
        guard let existing = itemsById[itemId] else { return }

        let updated = boundedSessionItem(apply(delta: delta, to: existing))
        itemsById[itemId] = updated
        updateTranscriptItem(itemId: itemId, turnId: turnId, item: updated)
    }

    private func updateTranscriptItem(itemId: String, turnId: String, item: SessionItem) {
        guard let entryId = itemEntryIdByItemId[itemId],
              let index = transcript.firstIndex(where: { $0.id == entryId }),
              case .item(let existing) = transcript[index] else {
            return
        }

        let updated = TranscriptItem(
            id: existing.id,
            turnId: turnId,
            itemId: itemId,
            card: projectCard(from: item),
            timestamp: existing.timestamp
        )
        transcript[index] = .item(updated)
    }

    private func apply(delta: ItemDelta, to item: SessionItem) -> SessionItem {
        switch (item, delta) {
        case let (.command(command), .output(content)):
            return .command(
                CommandItem(
                    id: command.id,
                    command: command.command,
                    cwd: command.cwd,
                    status: command.status,
                    output: appendDetail(command.output, delta: content),
                    exitCode: command.exitCode,
                    durationMs: command.durationMs
                )
            )

        case let (.tool(tool), .output(content)):
            return .tool(
                ToolItem(
                    id: tool.id,
                    name: tool.name,
                    status: tool.status,
                    input: tool.input,
                    output: appendDetail(tool.output, delta: content)
                )
            )

        case let (.message(message), .planText(content)):
            return .message(
                MessageItem(
                    id: message.id,
                    text: appendDetail(message.text, delta: content),
                    phase: message.phase
                )
            )

        case let (.thought(thought), .planText(content)):
            return .thought(
                ThoughtItem(
                    id: thought.id,
                    text: appendDetail(thought.text, delta: content)
                )
            )

        default:
            return item
        }
    }

    private func boundedSessionItem(_ item: SessionItem) -> SessionItem {
        switch item {
        case .command(let command):
            return .command(
                CommandItem(
                    id: command.id,
                    command: command.command,
                    cwd: command.cwd,
                    status: command.status,
                    output: trimDetail(command.output),
                    exitCode: command.exitCode,
                    durationMs: command.durationMs
                )
            )
        case .tool(let tool):
            return .tool(
                ToolItem(
                    id: tool.id,
                    name: tool.name,
                    status: tool.status,
                    input: tool.input,
                    output: trimDetail(tool.output)
                )
            )
        case .message(let message):
            return .message(
                MessageItem(
                    id: message.id,
                    text: trimDetail(message.text) ?? "",
                    phase: message.phase
                )
            )
        case .thought(let thought):
            return .thought(
                ThoughtItem(
                    id: thought.id,
                    text: trimDetail(thought.text) ?? ""
                )
            )
        case .file, .unknown:
            return item
        }
    }

    private func projectCard(from item: SessionItem) -> TranscriptItemCard {
        switch item {
        case .command(let command):
            let label = command.command.isEmpty ? "Command" : command.command.joined(separator: " ")
            return TranscriptItemCard(
                type: .command,
                label: label,
                status: command.status,
                detail: trimDetail(command.output)
            )

        case .file(let file):
            let paths = file.changes.map(\.path).filter { !$0.isEmpty }
            return TranscriptItemCard(
                type: .file,
                label: paths.isEmpty ? "File change" : paths.joined(separator: ", "),
                status: file.status,
                detail: nil
            )

        case .message(let message):
            return TranscriptItemCard(
                type: .message,
                label: summarize(message.text),
                status: nil,
                detail: nil
            )

        case .thought(let thought):
            return TranscriptItemCard(
                type: .thought,
                label: summarize(thought.text),
                status: nil,
                detail: nil
            )

        case .tool(let tool):
            return TranscriptItemCard(
                type: .tool,
                label: tool.name,
                status: tool.status,
                detail: trimDetail(tool.output)
            )

        case .unknown(let type, _):
            return TranscriptItemCard(
                type: .unknown,
                label: type,
                status: nil,
                detail: nil
            )
        }
    }

    private func summarize(_ text: String, maxLength: Int = 140) -> String {
        let compact = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard compact.count > maxLength else {
            return compact.isEmpty ? "(empty)" : compact
        }
        return String(compact.prefix(maxLength - 1)) + "…"
    }

    private func trimDetail(_ text: String?) -> String? {
        guard let text else { return nil }
        guard text.count > detailLimit else { return text }

        let keepCount = max(0, detailLimit - truncationSuffix.count)
        let tail = String(text.suffix(keepCount))
        return tail + truncationSuffix
    }

    private func appendDetail(_ existing: String?, delta: String) -> String {
        let base: String
        if let existing, existing.hasSuffix(truncationSuffix) {
            base = String(existing.dropLast(truncationSuffix.count))
        } else {
            base = existing ?? ""
        }

        return trimDetail(base + delta) ?? ""
    }

    private func itemIdentifier(_ item: SessionItem) -> String? {
        if let id = item.id, !id.isEmpty {
            return id
        }

        guard case .unknown(_, let payload) = item,
              let object = payload.objectValue,
              let id = object["id"]?.stringValue,
              !id.isEmpty else {
            return nil
        }

        return id
    }
}
