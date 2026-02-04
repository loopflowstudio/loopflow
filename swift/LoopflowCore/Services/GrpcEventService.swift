// GrpcEventService - EventServiceProtocol implementation using gRPC Subscribe streaming.

import Foundation
import GRPCCore
import GRPCNIOTransportHTTP2
import GRPCProtobuf
import os.log

private let logger = Logger(subsystem: "com.loopflow.concerto", category: "grpc-events")

public actor GrpcEventService: EventServiceProtocol {
    private var streamTask: Task<Void, Never>?
    private var onEvent: (@Sendable (LFDEvent) -> Void)?
    private var onConnectionChange: (@Sendable (Bool) -> Void)?
    private var patterns: [String] = []
    private var _isConnected = false
    private var reconnectAttempts = 0

    public init() {}

    public var isConnected: Bool { _isConnected }

    public func subscribe(
        patterns: [String],
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async {
        self.patterns = patterns
        self.onEvent = onEvent
        self.onConnectionChange = onConnectionChange

        LoggingService.lfd("GrpcEventService.subscribe: patterns=\(patterns)")
        startStreamLoop()
    }

    private func startStreamLoop() {
        streamTask?.cancel()
        reconnectAttempts = 0

        streamTask = Task { [weak self] in
            guard let self = self else { return }

            while !Task.isCancelled {
                do {
                    try await self.runStream()
                } catch {
                    await self.handleDisconnected()
                    logger.debug("stream error: \(error.localizedDescription)")
                }

                guard !Task.isCancelled else { return }

                // Backoff: fast for first 10 attempts, then slow
                let attempts = await self.reconnectAttempts
                let delay: Duration = attempts < 10 ? .milliseconds(500) : .seconds(5)
                try? await Task.sleep(for: delay)
                await self.incrementReconnectAttempts()
            }
        }
    }

    private func runStream() async throws {
        let patterns = self.patterns

        try await withGRPCClient(
            transport: .http2NIOPosix(
                target: .ipv4(host: lfdGrpcHost, port: lfdGrpcPort),
                transportSecurity: .plaintext
            )
        ) { grpcClient in
            let client = Loopflow_Control_V1_ControlService.Client(wrapping: grpcClient)

            var request = Loopflow_Control_V1_SubscribeRequest()
            request.patterns = patterns

            try await client.subscribe(request) { response in
                await self.handleConnected()

                for try await event in response.messages {
                    guard !Task.isCancelled else { return }
                    let lfdEvent = Self.convertEvent(event)
                    await self.handleEvent(lfdEvent)
                }
            }
        }
    }

    private func handleConnected() {
        guard !_isConnected else { return }
        logger.info("connected to lfd gRPC")
        LoggingService.append("gRPC connected", category: LoggingService.Category.lfd)
        _isConnected = true
        reconnectAttempts = 0
        onConnectionChange?(true)
    }

    private func handleDisconnected() {
        guard _isConnected else { return }
        logger.info("disconnected from lfd gRPC")
        LoggingService.append("gRPC disconnected", category: LoggingService.Category.lfd)
        _isConnected = false
        onConnectionChange?(false)
    }

    private func incrementReconnectAttempts() {
        reconnectAttempts += 1
    }

    private func handleEvent(_ event: LFDEvent) {
        onEvent?(event)
    }

    private nonisolated static func convertEvent(_ proto: Loopflow_Control_V1_Event) -> LFDEvent {
        let eventName = proto.event

        switch proto.payload {
        case .agentStarted(let e):
            return .session(SessionEvent(
                id: e.id,
                step: e.step,
                status: "started",
                worktree: e.worktree
            ))

        case .agentEnded(let e):
            return .session(SessionEvent(
                id: e.id,
                step: nil,
                status: e.status,
                worktree: nil
            ))

        case .outputLine(let e):
            return .output(OutputEvent(
                sessionId: e.agentID,
                text: e.text,
                timestamp: proto.timestamp.date
            ))

        case .worktreeUpdated(let e):
            return .worktree(WorktreeEvent(
                name: eventName,
                branch: e.branch,
                path: nil,
                reason: e.reason.reasonString,
                repo: e.repo,
                worktree: nil  // Full worktree status fetched separately if needed
            ))

        case .worktreePruned(let e):
            return .worktree(WorktreeEvent(
                name: eventName,
                branch: e.branch,
                path: nil,
                reason: "pruned",
                repo: e.repo,
                worktree: nil
            ))

        case .waveCreated(let e):
            return .wave(WaveEvent(
                name: eventName,
                waveId: e.waveID,
                stimulus: nil
            ))

        case .waveUpdated(let e):
            return .wave(WaveEvent(
                name: eventName,
                waveId: e.waveID,
                stimulus: nil
            ))

        case .waveDeleted(let e):
            return .wave(WaveEvent(
                name: eventName,
                waveId: e.waveID,
                stimulus: nil
            ))

        case .waveStarted(let e):
            return .wave(WaveEvent(
                name: eventName,
                waveId: e.waveID,
                stimulus: nil
            ))

        case .waveStopped(let e):
            return .wave(WaveEvent(
                name: eventName,
                waveId: e.waveID,
                stimulus: nil
            ))

        case .waveActivated(let e):
            return .wave(WaveEvent(
                name: eventName,
                waveId: e.waveID,
                stimulus: e.stimulus.stimulusString
            ))

        case .waveWaiting(let e):
            return .session(SessionEvent(
                id: e.agentID,
                step: e.step,
                status: "waiting",
                worktree: nil
            ))

        case .schedulerSlotAcquired, .schedulerSlotReleased:
            // Internal scheduler events, map to generic wave event
            return .wave(WaveEvent(name: eventName, waveId: nil, stimulus: nil))

        case .none:
            // Unknown event type, use event name to guess category
            if eventName.hasPrefix("wave.") {
                return .wave(WaveEvent(name: eventName, waveId: nil, stimulus: nil))
            } else if eventName.hasPrefix("session.") || eventName.hasPrefix("agent.") {
                return .session(SessionEvent(id: "", step: nil, status: nil, worktree: nil))
            } else {
                return .worktree(WorktreeEvent(
                    name: eventName,
                    branch: nil,
                    path: nil,
                    reason: nil,
                    repo: nil,
                    worktree: nil
                ))
            }
        }
    }

    public func disconnect() async {
        streamTask?.cancel()
        streamTask = nil
        onEvent = nil
        onConnectionChange = nil
        if _isConnected {
            _isConnected = false
            // Don't call onConnectionChange here since we cleared it
        }
    }
}

// MARK: - Proto Conversions

extension Loopflow_Control_V1_WorktreeChangeReason {
    var reasonString: String {
        switch self {
        case .unspecified: return "unknown"
        case .worktreeCommit: return "commit"
        case .worktreeCheckout: return "checkout"
        case .worktreeChanged: return "changed"
        case .worktreeDraftPrCreated: return "draft_pr_created"
        case .worktreeCiUpdated: return "ci_updated"
        case .worktreeMerged: return "merged"
        case .worktreePrStateChanged: return "pr_state_changed"
        case .UNRECOGNIZED: return "unrecognized"
        }
    }
}

extension Loopflow_Control_V1_StimulusKind {
    var stimulusString: String {
        switch self {
        case .unspecified: return "unspecified"
        case .stimulusOnce: return "once"
        case .stimulusLoop: return "loop"
        case .stimulusWatch: return "watch"
        case .stimulusCron: return "cron"
        case .UNRECOGNIZED: return "unrecognized"
        }
    }
}
