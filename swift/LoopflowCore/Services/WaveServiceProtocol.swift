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
    func createWave(
        name: String,
        repo: RepoTarget,
        flow: String,
        run: Bool,
        status: WaveStatus?
    ) async throws -> Wave
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave
    func deleteWave(_ id: String) async throws
    func cloneWave(_ id: String, name: String?) async throws -> Wave
    func ensureWaveAgent(waveId: String, overrides: RunOverrides?) async throws -> Session
    func getWaveAgentTree(waveId: String, activeOnly: Bool) async throws -> WaveAgentTree
    func addTrigger(_ waveId: String, signal: Trigger.Signal, flow: String?) async throws -> Trigger
    func removeTrigger(_ waveId: String, triggerId: String) async throws
    func createSession(
        harness: String,
        runId: String?,
        config: AgentSessionConfig
    ) async throws -> AgentSession
    func getSession(_ id: String) async throws -> AgentSession
    func stopSession(_ id: String) async throws -> AgentSession
    func stop(_ id: String) async throws
    func restartStep(_ id: String) async throws
    func listAttention(repo: RepoTarget) async throws -> [AttentionItem]
    func getAttention(_ id: String) async throws -> AttentionItem
    func markAttentionViewed(_ id: String) async throws -> AttentionItem
    func listSessions(repo: RepoTarget, activeOnly: Bool) async throws -> [Session]
    func createSession(
        waveId: String,
        flow: String,
        worktree: String,
        agent: String
    ) async throws -> SessionLaunchResponse
    func getSession(_ id: String) async throws -> Session
    func attachSession(_ id: String) async throws -> SessionConnectionInfo
    func startSession(_ id: String) async throws -> Session
    func cancelSession(_ id: String) async throws -> Session
    func landWave(_ id: String) async throws
    func nextWave(_ id: String) async throws -> String
    func listFlowsAndDirections(repo: RepoTarget) async throws -> WaveFlowsResult
    func listWorktrees(repo: RepoTarget) async throws -> [WorktreeInfo]
    func listRepos() async throws -> [RemoteRepo]
    func addRepo(path: String) async throws -> RemoteRepo
    func removeRepo(path: String) async throws
    func checkConnection() async throws
    func fileDiff(waveId: String, path: String) async throws -> String
}
