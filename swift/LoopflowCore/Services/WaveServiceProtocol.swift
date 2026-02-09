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
    func listWaves(repo: URL) async throws -> [Wave]
    func getWave(_ id: String) async throws -> Wave
    func createWave(name: String, repo: URL) async throws -> Wave
    func updateWave(_ id: String, config: WaveConfigUpdate) async throws -> Wave
    func deleteWave(_ id: String) async throws
    func cloneWave(_ id: String, name: String?) async throws -> Wave
    func run(_ id: String, overrides: RunOverrides?) async throws
    func stop(_ id: String) async throws
    func landWave(_ id: String) async throws
    func nextWave(_ id: String) async throws -> String
    func listFlowsAndDirections(repo: URL) async throws -> WaveFlowsResult
}
