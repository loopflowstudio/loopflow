#if os(iOS)
import Foundation
import LoopflowCore

enum LocalWaveAgentLauncher {
    static func attach(repoPath: String, waveName: String) async throws -> SessionConnectionInfo {
        throw WaveServiceError.commandFailed("Local wave agent launch is only available on macOS.")
    }

    static func sessionExists(repoPath: String, waveName: String) -> Bool {
        false
    }
}
#endif
