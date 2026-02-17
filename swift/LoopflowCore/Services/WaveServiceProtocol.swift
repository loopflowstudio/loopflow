// Protocol for wave service operations. Enables testing RepoState with mock services.

import Foundation

public struct WaveFlowsResult: Sendable {
    public var flows: [Flow]
    public var directions: [String]

    public init(flows: [Flow], directions: [String]) {
        self.flows = flows
        self.directions = directions
    }
}

public protocol WaveServiceProtocol: Sendable {
    func listWaves(repo: RepoTarget) async throws -> [Wave]
    func getWave(_ id: String) async throws -> Wave
    func createWave(name: String, repo: RepoTarget, schema: String?) async throws -> Wave
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave
    func deleteWave(_ id: String) async throws
    func cloneWave(_ id: String, name: String?) async throws -> Wave
    func run(_ id: String, overrides: RunOverrides?) async throws
    func addStimulus(_ waveId: String, kind: Stimulus.Kind, cron: String?) async throws -> Stimulus
    func removeStimulus(_ waveId: String, stimulusId: String) async throws
    func listMemoryBlocks(waveId: String) async throws -> [ChatMemoryBlock]
    func upsertMemoryBlock(
        waveId: String,
        name: String,
        content: String,
        position: Int?
    ) async throws -> ChatMemoryBlock
    func deleteMemoryBlock(waveId: String, name: String) async throws
    func startChat(
        waveId: String,
        message: String,
        memoryBlocks: [ChatMemoryBlock]
    ) async throws
    func streamChatEvents(
        waveId: String
    ) -> AsyncThrowingStream<ChatTurnEvent, Error>
    func stop(_ id: String) async throws
    func restartStep(_ id: String) async throws
    func landWave(_ id: String) async throws
    func nextWave(_ id: String) async throws -> String
    func listFlowsAndDirections(repo: RepoTarget) async throws -> WaveFlowsResult
    func listWaveSchemas(repo: RepoTarget) async throws -> [WaveSchema]
    func listWorktrees(repo: RepoTarget) async throws -> [WorktreeInfo]
    func listRepos() async throws -> [RemoteRepo]
    func checkConnection() async throws
}

public extension WaveServiceProtocol {
    func createWave(name: String, repo: RepoTarget) async throws -> Wave {
        try await createWave(name: name, repo: repo, schema: nil)
    }
}
