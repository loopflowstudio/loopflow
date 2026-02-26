// Protocol for wave service operations. Enables testing RepoState with mock services.

import Foundation

public struct WaveFlowsResult: Sendable {
    public var flows: [Flow]
    public var directions: [String]
    public var supportedHarnesses: [String]

    public init(flows: [Flow], directions: [String], supportedHarnesses: [String] = []) {
        self.flows = flows
        self.directions = directions
        self.supportedHarnesses = supportedHarnesses
    }
}

public protocol WaveServiceProtocol: Sendable {
    func listWaves(repo: RepoTarget) async throws -> [Wave]
    func getWave(_ id: String) async throws -> Wave
    func createWave(name: String, repo: RepoTarget) async throws -> Wave
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave
    func deleteWave(_ id: String) async throws
    func cloneWave(_ id: String, name: String?) async throws -> Wave
    func run(_ id: String, overrides: RunOverrides?) async throws
    func addStimulus(_ waveId: String, kind: Stimulus.Kind, cron: String?) async throws -> Stimulus
    func removeStimulus(_ waveId: String, stimulusId: String) async throws
    func createSession(
        harness: String,
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
    func stop(_ id: String) async throws
    func restartStep(_ id: String) async throws
    func landWave(_ id: String) async throws
    func nextWave(_ id: String) async throws -> String
    func listFlowsAndDirections(repo: RepoTarget) async throws -> WaveFlowsResult
    func listWorktrees(repo: RepoTarget) async throws -> [WorktreeInfo]
    func listRepos() async throws -> [RemoteRepo]
    func addRepo(path: String) async throws -> RemoteRepo
    func removeRepo(path: String) async throws
    func checkConnection() async throws
}
