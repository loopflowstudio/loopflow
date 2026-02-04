import Foundation

public struct WaveConfigUpdate: Sendable {
    public var name: String?
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?
    public var stimulus: Stimulus?
    public var paused: Bool?

    public init(
        name: String? = nil,
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil,
        paused: Bool? = nil
    ) {
        self.name = name
        self.area = area
        self.direction = direction
        self.flow = flow
        self.stimulus = stimulus
        self.paused = paused
    }
}

public struct RunOverrides: Sendable {
    public var area: [String]?
    public var direction: [String]?
    public var flow: String?
    public var stimulus: Stimulus?

    public init(
        area: [String]? = nil,
        direction: [String]? = nil,
        flow: String? = nil,
        stimulus: Stimulus? = nil
    ) {
        self.area = area
        self.direction = direction
        self.flow = flow
        self.stimulus = stimulus
    }
}

public struct ConnectionInfo: Sendable {
    public let worktree: String
    public let step: String
    public let agentId: String
    public let promptFile: String
    public let waveRunId: String?
    public let stepIndex: Int

    public init(
        worktree: String,
        step: String,
        agentId: String,
        promptFile: String,
        waveRunId: String?,
        stepIndex: Int
    ) {
        self.worktree = worktree
        self.step = step
        self.agentId = agentId
        self.promptFile = promptFile
        self.waveRunId = waveRunId
        self.stepIndex = stepIndex
    }
}

public protocol WaveServiceProtocol: Sendable {
    func listWaves(repo: URL) async throws -> [Wave]
    func createWave(name: String, repo: URL) async throws -> Wave
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave
    func deleteWave(_ id: String) async throws
    func cloneWave(_ id: String, name: String?) async throws -> Wave
    func run(_ id: String, overrides: RunOverrides?) async throws
    func stop(_ id: String) async throws
    func connect(_ id: String) async throws -> ConnectionInfo
    func listWaveRuns(waveId: String?, repo: URL?, limit: Int) async throws -> [WaveRun]
    func collapsePRs(_ id: String) async throws -> CollapsePRsResult
    func checkAvailability() async -> Bool
    func connectLfd() async throws
}

public protocol EventServiceProtocol: Sendable {
    func subscribe(
        patterns: [String],
        onEvent: @escaping @Sendable (LFDEvent) -> Void,
        onConnectionChange: @escaping @Sendable (Bool) -> Void
    ) async
    func disconnect() async
    var isConnected: Bool { get async }
}
