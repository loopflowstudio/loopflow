#if os(macOS)
// Type definitions for embedded Ghostty terminal.

import Foundation

#if canImport(GhosttyKit)
import GhosttyKit
#endif

enum TerminalStatus: Equatable {
    case initializing
    case running
    case completed(exitCode: Int32)
    case failed(error: String)
}

struct GhosttySession: Identifiable {
    let id: String
    let worktree: String
    let command: [String]
    var status: TerminalStatus

    #if canImport(GhosttyKit)
    var surface: ghostty_surface_t?
    #endif
}

#endif
