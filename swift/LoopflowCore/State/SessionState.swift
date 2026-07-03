import Foundation

public enum MessageRole {
    case user
    case assistant
    case error
    case system
}

public struct SessionMessage: Identifiable, Equatable {
    public let id: UUID
    public let role: MessageRole
    public let content: String
    public let timestamp: Date

    public init(id: UUID = UUID(), role: MessageRole, content: String, timestamp: Date = Date()) {
        self.id = id
        self.role = role
        self.content = content
        self.timestamp = timestamp
    }
}

public enum SessionItemType: Equatable {
    case command
    case file
    case message
    case thought
    case tool
    case unknown
}

public struct TranscriptItemCard: Equatable {
    public let type: SessionItemType
    public let label: String
    public let status: ItemStatus?
    public let detail: String?

    public init(type: SessionItemType, label: String, status: ItemStatus?, detail: String?) {
        self.type = type
        self.label = label
        self.status = status
        self.detail = detail
    }
}

public struct TranscriptItem: Identifiable, Equatable {
    public let id: UUID
    public let turnId: String
    public let itemId: String
    public let card: TranscriptItemCard
    public let timestamp: Date

    public init(
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

public enum TranscriptEntry: Identifiable, Equatable {
    case message(SessionMessage)
    case item(TranscriptItem)

    public var id: UUID {
        switch self {
        case .message(let message):
            return message.id
        case .item(let item):
            return item.id
        }
    }

    public var timestamp: Date {
        switch self {
        case .message(let message):
            return message.timestamp
        case .item(let item):
            return item.timestamp
        }
    }
}

public enum TranscriptGroup: Identifiable, Equatable {
    case single(TranscriptEntry)
    case toolRun([TranscriptItem])

    public var id: UUID {
        switch self {
        case .single(let entry):
            return entry.id
        case .toolRun(let items):
            return items[0].id
        }
    }
}

public protocol SessionService: Sendable {
    func createSession(
        harness: String,
        runId: String?,
        config: AgentSessionConfig
    ) async throws -> AgentSession
    func getSession(_ id: String) async throws -> AgentSession
    func stopSession(_ id: String) async throws -> AgentSession
}

extension LocalWaveService: SessionService {}

public enum TurnState {
    case idle
    case running
    case completed
    case failed
}

public enum StreamPhase {
    case idle
    case replaying
    case live
    case ending
}

@MainActor
@Observable
public final class SessionState {
    public let waveId: String

    public private(set) var transcript: [TranscriptEntry] = []
    public private(set) var groupedTranscript: [TranscriptGroup] = []
    public private(set) var latestAssistantMessageId: UUID?
    public private(set) var timestampLabels: [UUID: String] = [:]
    public var turnState: TurnState = .idle
    public var streamPhase: StreamPhase = .idle
    public var awaitingSessionJoin: Bool = false
    public var suggestedActions: [SuggestedAction] = []
    public var contextSnapshot: ContextSnapshot?

    public private(set) var itemsById: [String: SessionItem] = [:]

    private let sessionHarness: String
    private let sessionRunId: String?
    private var sessionConfig: AgentSessionConfig
    private let waveService: any SessionService
    private let userDefaults: UserDefaults
    private let detailLimit = 16_000
    private let truncationSuffix = "…truncated"

    private var sessionId: String?

    private var itemEntryIdByItemId: [String: UUID] = [:]
    private var assistantEntryIdByTurnId: [String: UUID] = [:]
    private var messageEntryIdByItemId: [String: UUID] = [:]
    private var transcriptIndexById: [UUID: Int] = [:]

    public init(
        waveId: String,
        sessionHarness: String = "claude",
        sessionRunId: String? = nil,
        sessionConfig: AgentSessionConfig,
        waveService: any SessionService = LocalWaveService(),
        userDefaults: UserDefaults = .standard
    ) {
        self.waveId = waveId
        self.sessionHarness = sessionHarness
        self.sessionRunId = sessionRunId
        self.sessionConfig = sessionConfig
        self.waveService = waveService
        self.userDefaults = userDefaults
    }

    public var isLoading: Bool {
        awaitingSessionJoin || turnState == .running || streamPhase == .replaying || streamPhase == .ending
    }

    public var canSend: Bool {
        sessionId == nil && turnState != .running && streamPhase != .ending
    }

    public var canEndSession: Bool {
        sessionId != nil && streamPhase != .ending
    }

    public func onAppear() async {
        if let sessionId {
            awaitingSessionJoin = false
            _ = sessionId
            return
        }

        await reconnectIfNeeded()
    }

    public func joinSession(_ id: String) {
        let trimmed = id.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        guard sessionId != trimmed else { return }

        resetForReplay()
        sessionId = trimmed
        persistSessionId(trimmed)
        streamPhase = .idle
        turnState = .idle
        awaitingSessionJoin = false
    }

    public func setAwaitingSession(_ awaiting: Bool) {
        guard sessionId == nil else {
            awaitingSessionJoin = false
            return
        }
        awaitingSessionJoin = awaiting
    }

    public func seedInitialUserMessage(_ rawText: String) {
        let text = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        _ = upsertMessageBubble(
            item: .message(MessageItem(id: "msg_seed_user_prompt", text: text, phase: "user")),
            isCompletion: true
        )
    }

    public func onDisappear() {
        if streamPhase != .ending {
            streamPhase = .idle
        }
    }

    public func send(_ rawText: String) async {
        let text = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }

        guard sessionId == nil else {
            appendMessage(role: .system, content: "Input is no longer supported for joined sessions.")
            return
        }

        guard turnState != .running && streamPhase != .ending else { return }

        clearSuggestedActions()
        appendMessage(role: .user, content: text)
        turnState = .running
        awaitingSessionJoin = false

        do {
            _ = try await ensureSession()
            turnState = .completed
        } catch {
            appendMessage(role: .error, content: error.localizedDescription)
            turnState = .failed
        }
    }

    public func endSession() async {
        guard let sessionId else { return }

        streamPhase = .ending

        do {
            _ = try await waveService.stopSession(sessionId)
        } catch {
            appendMessage(role: .error, content: error.localizedDescription)
        }

        self.sessionId = nil
        persistSessionId(nil)
        turnState = .idle
        streamPhase = .idle
        clearSuggestedActions()
        appendMessage(role: .system, content: "Session ended")
    }

    public func configureClientContext(compact: Bool) {
        guard sessionId == nil else { return }
        sessionConfig.clientHasUI = true
        sessionConfig.clientCompact = compact
    }

    public func sendSuggestedAction(_ action: SuggestedAction) async {
        clearSuggestedActions()
        await send(action.label)
    }

    public func reconnectIfNeeded() async {
        guard sessionId == nil else { return }
        guard let storedSessionId = userDefaults.string(forKey: sessionIdKey) else { return }

        streamPhase = .replaying

        do {
            let session = try await waveService.getSession(storedSessionId)
            if isSessionTerminalStatus(session.status) {
                persistSessionId(nil)
                streamPhase = .idle
                awaitingSessionJoin = false
                return
            }

            sessionId = session.id
            resetForReplay()
            appendMessage(role: .system, content: "Session restored")
            streamPhase = .idle
        } catch {
            persistSessionId(nil)
            streamPhase = .idle
            awaitingSessionJoin = false
        }
    }

    private var sessionIdKey: String {
        "session.\(waveId)"
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
            harness: sessionHarness,
            runId: sessionRunId,
            config: sessionConfig
        )
        sessionId = session.id
        persistSessionId(session.id)
        appendMessage(role: .system, content: "Session started")
        awaitingSessionJoin = false

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
                    domain: "SessionState",
                    code: 1,
                    userInfo: [NSLocalizedDescriptionKey: "Session ended before becoming active (\(session.status))"]
                )
            }

            if attempt < 19 {
                try await Task.sleep(for: .milliseconds(150))
            }
        }

        throw NSError(
            domain: "SessionState",
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

    private func resetForReplay() {
        setTranscript([])
        turnState = .idle
        resetSessionCaches()
    }

    private func resetSessionCaches() {
        itemsById.removeAll()
        itemEntryIdByItemId.removeAll()
        assistantEntryIdByTurnId.removeAll()
        messageEntryIdByItemId.removeAll()
        transcriptIndexById.removeAll()
        clearSuggestedActions()
        contextSnapshot = nil
    }

    private func appendMessage(role: MessageRole, content: String) {
        appendTranscriptEntry(.message(SessionMessage(role: role, content: content)))
    }

    private func upsertMessageBubble(item: SessionItem, isCompletion: Bool) -> Bool {
        guard case .message(let message) = item else { return false }
        guard let role = roleForMessagePhase(message.phase) else { return false }

        let text = message.text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return true }

        if let itemId = normalizedItemId(message.id) {
            if let entryId = messageEntryIdByItemId[itemId],
               let index = transcriptIndexById[entryId],
               case .message(let existing) = transcript[index] {
                updateTranscriptEntry(id: entryId) { _ in
                    .message(
                        SessionMessage(
                            id: existing.id,
                            role: role,
                            content: text,
                            timestamp: existing.timestamp
                        )
                    )
                }
                return true
            }

            let messageEntry = SessionMessage(role: role, content: text)
            appendTranscriptEntry(.message(messageEntry))
            messageEntryIdByItemId[itemId] = messageEntry.id
            return true
        }

        guard isCompletion else { return true }
        appendTranscriptEntry(.message(SessionMessage(role: role, content: text)))
        return true
    }

    private func roleForMessagePhase(_ phase: String?) -> MessageRole? {
        guard let phase else { return nil }
        let normalized = phase.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if normalized == "user" || normalized == "input" || normalized == "prompt" {
            return .user
        }
        return nil
    }

    private func normalizedItemId(_ rawId: String) -> String? {
        let trimmed = rawId.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private func appendAssistantDelta(turnId: String, delta: String) {
        if let entryId = assistantEntryIdByTurnId[turnId],
           let index = transcriptIndexById[entryId],
           case .message(let message) = transcript[index],
           message.role == .assistant {
            updateTranscriptEntry(id: entryId) { _ in
                .message(
                    SessionMessage(
                        id: message.id,
                        role: .assistant,
                        content: message.content + delta,
                        timestamp: message.timestamp
                    )
                )
            }
            return
        }

        let message = SessionMessage(role: .assistant, content: delta)
        appendTranscriptEntry(.message(message))
        assistantEntryIdByTurnId[turnId] = message.id
    }

    private func applySuggestedActions(_ payloads: [SuggestedActionPayload]) {
        let sanitized = payloads
            .compactMap { payload -> SuggestedAction? in
                let label = payload.label.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !label.isEmpty else { return nil }
                let description = payload.description?.trimmingCharacters(in: .whitespacesAndNewlines)
                return SuggestedAction(
                    label: label,
                    description: (description?.isEmpty == false) ? description : nil
                )
            }
            .prefix(4)

        suggestedActions = Array(sanitized)
    }

    private func clearSuggestedActions() {
        suggestedActions.removeAll()
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
            appendTranscriptEntry(.item(entry))
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
              let index = transcriptIndexById[entryId],
              case .item(let existing) = transcript[index] else {
            return
        }

        updateTranscriptEntry(id: entryId) { _ in
            .item(
                TranscriptItem(
                    id: existing.id,
                    turnId: turnId,
                    itemId: itemId,
                    card: projectCard(from: item),
                    timestamp: existing.timestamp
                )
            )
        }
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
            let diffs = file.changes.compactMap(\.diff).filter { !$0.isEmpty }
            let combinedDiff = diffs.joined(separator: "\n")
            return TranscriptItemCard(
                type: .file,
                label: paths.isEmpty ? "File change" : paths.joined(separator: ", "),
                status: file.status,
                detail: combinedDiff.isEmpty ? nil : combinedDiff
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

    private func setTranscript(_ entries: [TranscriptEntry]) {
        transcript = entries
        rebuildTranscriptIndex()
        rebuildDerivedTranscriptState()
    }

    private func appendTranscriptEntry(_ entry: TranscriptEntry) {
        transcript.append(entry)
        transcriptIndexById[entry.id] = transcript.count - 1
        appendDerivedTranscriptState(entry)
    }

    private func updateTranscriptEntry(id: UUID, transform: (TranscriptEntry) -> TranscriptEntry) {
        guard let index = transcriptIndexById[id] else { return }
        let previous = transcript[index]
        let updated = transform(previous)
        transcript[index] = updated

        if updated.id != id {
            transcriptIndexById.removeValue(forKey: id)
            transcriptIndexById[updated.id] = index
        }

        updateDerivedTranscriptState(previous: previous, updated: updated)
    }

    private func rebuildTranscriptIndex() {
        transcriptIndexById = Dictionary(uniqueKeysWithValues: transcript.enumerated().map { ($0.element.id, $0.offset) })
    }

    private func rebuildDerivedTranscriptState() {
        groupedTranscript = buildTranscriptGroups(from: transcript)
        latestAssistantMessageId = latestAssistantMessageID(in: transcript)
        timestampLabels = messageTimestampLabels(for: transcript)
    }

    private func appendDerivedTranscriptState(_ entry: TranscriptEntry) {
        if case .message(let message) = entry,
           message.role == .assistant {
            latestAssistantMessageId = message.id
        }

        updateTimestampLabelsAfterAppend(entry)
        appendTranscriptGroup(entry, to: &groupedTranscript)
    }

    private func updateTimestampLabelsAfterAppend(_ entry: TranscriptEntry) {
        guard case .message(let message) = entry,
              let label = messageTimestampLabel(
                for: message,
                previousMessage: latestVisibleMessage(in: transcript.dropLast().reversed())
              ) else {
            return
        }
        timestampLabels[message.id] = label
    }

    private func updateDerivedTranscriptState(previous: TranscriptEntry, updated: TranscriptEntry) {
        if shouldRebuildDerivedTranscriptState(previous: previous, updated: updated) {
            rebuildDerivedTranscriptState()
            return
        }

        if case .message(let message) = updated,
           message.role == .assistant {
            latestAssistantMessageId = message.id
        } else if latestAssistantMessageId == previous.id {
            latestAssistantMessageId = latestAssistantMessageID(in: transcript)
        }

        guard let groupIndex = groupedTranscript.lastIndex(where: { groupContainsEntry($0, id: previous.id) }) else {
            rebuildDerivedTranscriptState()
            return
        }

        switch groupedTranscript[groupIndex] {
        case .single:
            groupedTranscript[groupIndex] = .single(updated)
        case .toolRun(let items):
            guard case .item(let updatedItem) = updated,
                  let itemIndex = items.firstIndex(where: { $0.id == previous.id }) else {
                rebuildDerivedTranscriptState()
                return
            }

            var nextItems = items
            nextItems[itemIndex] = updatedItem
            groupedTranscript[groupIndex] = .toolRun(nextItems)
        }
    }

    private func shouldRebuildDerivedTranscriptState(previous: TranscriptEntry, updated: TranscriptEntry) -> Bool {
        switch (previous, updated) {
        case (.message(let previousMessage), .message(let updatedMessage)):
            return previousMessage.role != updatedMessage.role || previousMessage.timestamp != updatedMessage.timestamp
        case (.item(let previousItem), .item(let updatedItem)):
            return previousItem.card.type.isToolLike != updatedItem.card.type.isToolLike
        default:
            return true
        }
    }

    private func groupContainsEntry(_ group: TranscriptGroup, id: UUID) -> Bool {
        switch group {
        case .single(let entry):
            return entry.id == id
        case .toolRun(let items):
            return items.contains(where: { $0.id == id })
        }
    }
}

func latestAssistantMessageID(in transcript: [TranscriptEntry]) -> UUID? {
    transcript.reversed().compactMap { entry -> UUID? in
        guard case .message(let message) = entry,
              message.role == .assistant else {
            return nil
        }
        return message.id
    }.first
}

func buildTranscriptGroups(from transcript: [TranscriptEntry]) -> [TranscriptGroup] {
    var groups: [TranscriptGroup] = []

    for entry in transcript {
        appendTranscriptGroup(entry, to: &groups)
    }

    return groups
}

func messageTimestampLabels(for transcript: [TranscriptEntry]) -> [UUID: String] {
    var labels: [UUID: String] = [:]
    var previousMessage: SessionMessage?

    for entry in transcript {
        guard case .message(let message) = entry else {
            continue
        }

        if let label = messageTimestampLabel(for: message, previousMessage: previousMessage) {
            labels[message.id] = label
        }

        if message.role != .system {
            previousMessage = message
        }
    }

    return labels
}

private func appendTranscriptGroup(_ entry: TranscriptEntry, to groups: inout [TranscriptGroup]) {
    switch entry {
    case .item(let item) where item.card.type.isToolLike:
        guard let lastGroup = groups.last else {
            groups.append(.single(.item(item)))
            return
        }

        switch lastGroup {
        case .toolRun(let items):
            groups[groups.count - 1] = .toolRun(items + [item])
        case .single(.item(let existing)) where existing.card.type.isToolLike:
            groups[groups.count - 1] = .toolRun([existing, item])
        default:
            groups.append(.single(.item(item)))
        }
    default:
        groups.append(.single(entry))
    }
}

private func latestVisibleMessage<S: Sequence>(in entries: S) -> SessionMessage? where S.Element == TranscriptEntry {
    for entry in entries {
        guard case .message(let message) = entry,
              message.role != .system else {
            continue
        }
        return message
    }
    return nil
}

private func messageTimestampLabel(for message: SessionMessage, previousMessage: SessionMessage?) -> String? {
    guard message.role != .system else { return nil }
    guard let previousMessage else {
        return formatMessageTimestamp(message.timestamp)
    }

    if message.role == .user {
        return formatMessageTimestamp(message.timestamp)
    }

    return timestampLabel(
        forGapSincePreviousMessage: message.timestamp.timeIntervalSince(previousMessage.timestamp),
        timestamp: message.timestamp
    )
}

func timestampLabel(forGapSincePreviousMessage gap: TimeInterval, timestamp: Date) -> String? {
    if gap <= 60 {
        return nil
    }
    if gap > 3600 {
        return formatMessageTimestamp(timestamp)
    }
    let minutes = max(1, Int(gap / 60))
    return "\(minutes)m ago"
}

func formatMessageTimestamp(_ timestamp: Date) -> String {
    timestamp.formatted(date: .omitted, time: .shortened)
}

private extension SessionItemType {
    var isToolLike: Bool {
        switch self {
        case .command, .tool, .file:
            return true
        case .message, .thought, .unknown:
            return false
        }
    }
}
