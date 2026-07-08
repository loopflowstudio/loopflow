#if os(iOS)
import Foundation

enum LocalWaveAgentLauncher {
    static func sessionExists(repoPath: String, waveName: String) -> Bool {
        false
    }
}
#endif
