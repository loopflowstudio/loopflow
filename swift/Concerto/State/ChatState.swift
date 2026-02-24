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

protocol ChatService: Sendable {
    func createSession(
        provider: String,
        waveRunId: String?,
        config: AgentSessionConfig
    ) async throws -> AgentSession
    func sendSessionInput(sessionId: String, content: String) async throws -> AgentSession
    func streamSessionEvents(
        sessionId: String,
        afterSeq: Int?
    ) -> AsyncThrowingStream<AgentSessionEventEnvelope, Error>
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
    let waveId: String

    var messages: [ChatMessage] = []
    var turnState: ChatTurnState = .idle

    private let waveService: any ChatService
    private var sessionId: String?
    private var lastEventSeq: Int?
    private var currentTurnId: String?

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

    func send(_ rawText: String) async {
        guard turnState != .running else { return }

        let text = rawText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }

        messages.append(ChatMessage(role: .user, content: text))
        turnState = .running

        do {
            let sessionId = try await ensureSession()
            _ = try await waveService.sendSessionInput(sessionId: sessionId, content: text)

            var assistantMessageIndex: Int?
            var terminalStatus: String?

            eventLoop: for try await envelope in waveService.streamSessionEvents(
                sessionId: sessionId,
                afterSeq: lastEventSeq
            ) {
                if let seq = envelope.seq {
                    lastEventSeq = max(lastEventSeq ?? seq, seq)
                }

                switch envelope.event {
                case .turnStarted(let turnId):
                    currentTurnId = turnId
                    assistantMessageIndex = nil

                case .textDelta(let turnId, let delta):
                    guard currentTurnId == nil || turnId == currentTurnId else { continue }
                    guard !delta.isEmpty else { continue }
                    if let index = assistantMessageIndex,
                       messages.indices.contains(index),
                       messages[index].role == .assistant {
                        let existing = messages[index]
                        messages[index] = ChatMessage(
                            id: existing.id,
                            role: .assistant,
                            content: existing.content + delta,
                            timestamp: existing.timestamp
                        )
                    } else {
                        messages.append(ChatMessage(role: .assistant, content: delta))
                        assistantMessageIndex = messages.indices.last
                    }

                case .turnCompleted(let turnId, let status):
                    guard currentTurnId == nil || turnId == currentTurnId else { continue }
                    terminalStatus = status
                    currentTurnId = nil
                    break eventLoop

                case .error(_, let message):
                    let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
                    if !trimmed.isEmpty {
                        messages.append(ChatMessage(role: .error, content: trimmed))
                    }

                case .reasoningDelta, .statusChanged, .other:
                    continue
                }
            }

            switch terminalStatus {
            case "completed":
                turnState = .completed
            case "failed", "interrupted":
                turnState = .failed
            default:
                turnState = .failed
            }
        } catch {
            messages.append(ChatMessage(role: .error, content: error.localizedDescription))
            turnState = .failed
        }
    }

    private func ensureSession() async throws -> String {
        if let existing = sessionId {
            return existing
        }

        let session = try await waveService.createSession(
            provider: "claude",
            waveRunId: nil,
            config: AgentSessionConfig()
        )
        sessionId = session.id
        return session.id
    }
}
