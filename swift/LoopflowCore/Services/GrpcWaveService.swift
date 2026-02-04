// GrpcWaveService - WaveServiceProtocol implementation using gRPC.

import Foundation
import GRPCCore
import GRPCNIOTransportHTTP2
import GRPCProtobuf
import SwiftProtobuf

public let lfdGrpcHost = "127.0.0.1"
public let lfdGrpcPort = 50051

public final class GrpcWaveService: WaveServiceProtocol, @unchecked Sendable {
    public init() {}

    // MARK: - Connection Helper

    private func withClient<T: Sendable>(
        _ operation: @Sendable @escaping (Loopflow_Control_V1_ControlService.Client<HTTP2ClientTransport.Posix>) async throws -> T
    ) async throws -> T {
        try await withGRPCClient(
            transport: .http2NIOPosix(
                target: .ipv4(host: lfdGrpcHost, port: lfdGrpcPort),
                transportSecurity: .plaintext
            )
        ) { grpcClient in
            let client = Loopflow_Control_V1_ControlService.Client(wrapping: grpcClient)
            return try await operation(client)
        }
    }

    // MARK: - WaveServiceProtocol

    public func listWaves(repo: URL) async throws -> [Wave] {
        try await withClient { client in
            var request = Loopflow_Control_V1_ListWavesRequest()
            request.repo = repo.path
            let response = try await client.listWaves(request)
            return response.waves.map { Wave(from: $0) }
        }
    }

    public func createWave(name: String, repo: URL) async throws -> Wave {
        try await withClient { client in
            var request = Loopflow_Control_V1_CreateWaveRequest()
            request.repo = repo.path
            request.name = name
            request.flow = "design"
            request.direction = ["default"]
            request.area = ["."]

            let response = try await client.createWave(request)
            guard response.hasWave else {
                throw WaveServiceError.commandFailed("No wave in response")
            }
            return Wave(from: response.wave)
        }
    }

    public func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave {
        try await withClient { client in
            var request = Loopflow_Control_V1_UpdateWaveRequest()
            request.waveID = id
            if let flow = config.flow { request.flow = flow }
            if let direction = config.direction { request.direction = direction }
            if let area = config.area { request.area = area }
            if let paused = config.paused { request.paused = paused }

            let response = try await client.updateWave(request)
            guard response.hasWave else {
                throw WaveServiceError.commandFailed("No wave in response")
            }
            return Wave(from: response.wave)
        }
    }

    public func deleteWave(_ id: String) async throws {
        try await withClient { client in
            var request = Loopflow_Control_V1_DeleteWaveRequest()
            request.waveID = id
            _ = try await client.deleteWave(request)
        }
    }

    public func cloneWave(_ id: String, name: String?) async throws -> Wave {
        try await withClient { client in
            var request = Loopflow_Control_V1_CloneWaveRequest()
            request.waveID = id
            if let name = name { request.name = name }

            let response = try await client.cloneWave(request)
            guard response.hasWave else {
                throw WaveServiceError.commandFailed("No wave in response")
            }
            return Wave(from: response.wave)
        }
    }

    public func run(_ id: String, overrides: RunOverrides?) async throws {
        try await withClient { client in
            var request = Loopflow_Control_V1_RunWaveRequest()
            request.waveID = id
            if let overrides = overrides {
                if let area = overrides.area { request.area = area }
                if let direction = overrides.direction { request.direction = direction }
                if let flow = overrides.flow { request.flow = flow }
            }

            let response = try await client.runWave(request)
            if !response.started {
                throw WaveServiceError.commandFailed("Failed to start wave")
            }
        }
    }

    public func stop(_ id: String) async throws {
        try await withClient { client in
            var request = Loopflow_Control_V1_StopWaveRequest()
            request.waveID = id
            _ = try await client.stopWave(request)
        }
    }

    public func connect(_ id: String) async throws -> ConnectionInfo {
        try await withClient { client in
            var request = Loopflow_Control_V1_ConnectWaveRequest()
            request.waveID = id

            let response = try await client.connectWave(request)
            return ConnectionInfo(
                worktree: response.worktree,
                step: response.step,
                agentId: response.agentID,
                promptFile: response.promptFile,
                waveRunId: response.waveRunID.isEmpty ? nil : response.waveRunID,
                stepIndex: Int(response.stepIndex)
            )
        }
    }

    public func listWaveRuns(waveId: String?, repo: URL?, limit: Int) async throws -> [WaveRun] {
        try await withClient { client in
            var request = Loopflow_Control_V1_ListWaveRunsRequest()
            if let waveId = waveId { request.waveID = waveId }
            request.limit = UInt32(limit)

            let response = try await client.listWaveRuns(request)
            return response.runs.map { WaveRun(from: $0) }
        }
    }

    public func collapsePRs(_ id: String) async throws -> CollapsePRsResult {
        // Note: This RPC doesn't exist in proto yet - needs HTTP fallback or proto extension
        throw WaveServiceError.commandFailed("collapsePRs not implemented in gRPC")
    }

    public func checkAvailability() async -> Bool {
        do {
            return try await withClient { client in
                let request = Loopflow_Control_V1_GetStatusRequest()
                _ = try await client.getStatus(request)
                return true
            }
        } catch {
            return false
        }
    }

    public func connectLfd() async throws {
        // This shells out to `lfd install` - keep as shell command
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = ["lfd", "install"]
        try process.run()
        process.waitUntilExit()

        if process.terminationStatus != 0 {
            throw WaveServiceError.commandFailed("lfd install failed with status \(process.terminationStatus)")
        }
    }
}

// MARK: - Proto Conversions

extension Wave {
    init(from proto: Loopflow_Control_V1_Wave) {
        self.init(
            id: proto.id,
            name: proto.name,
            area: proto.area.isEmpty ? nil : Array(proto.area),
            direction: proto.direction.isEmpty ? nil : Array(proto.direction),
            flow: proto.flow,
            repo: proto.repo,
            stimulus: Stimulus(kind: .manual),  // Default, stimulus is separate entity now
            paused: proto.paused,
            status: .idle,  // Status comes from WaveRun, not Wave
            iteration: 0,
            createdAt: proto.createdAt.date
        )
    }
}

extension WaveRun {
    init(from proto: Loopflow_Control_V1_WaveRun) {
        let status: WaveRunStatus
        switch proto.status {
        case .waveRunPending: status = .pending
        case .waveRunRunning: status = .running
        case .waveRunWaiting: status = .pending  // Map waiting to pending (no waiting in Swift enum)
        case .waveRunCompleted: status = .completed
        case .waveRunFailed: status = .failed
        default: status = .pending
        }

        self.init(
            id: proto.id,
            waveId: proto.waveID.isEmpty ? nil : proto.waveID,
            flow: "",  // Not in proto WaveRun
            area: "",
            repo: "",
            direction: [],
            status: status,
            iteration: Int(proto.iteration),
            worktree: proto.worktree.isEmpty ? nil : proto.worktree,
            branch: proto.branch.isEmpty ? nil : proto.branch,
            startedAt: proto.hasStartedAt ? proto.startedAt.date : nil,
            endedAt: proto.hasEndedAt ? proto.endedAt.date : nil
        )
    }
}

extension SwiftProtobuf.Google_Protobuf_Timestamp {
    var date: Date {
        Date(timeIntervalSince1970: TimeInterval(seconds) + TimeInterval(nanos) / 1_000_000_000)
    }
}
