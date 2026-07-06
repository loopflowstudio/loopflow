// The local macOS registry query: `RegistryQuery` backed by a launcher-resolved
// `lf`. Discovery and history are `lf` subprocesses over `lfdb`, run off the
// main actor so a slow query never stalls the UI.

#if os(macOS)
import Foundation
import LoopflowCore

enum RegistryQueryLocal {
    static let shared = RegistryQuery { args, cwd in
        try await Task.detached(priority: .userInitiated) {
            try LocalWaveAgentLauncher.queryLf(args, cwd: cwd)
        }.value
    }
}
#endif
