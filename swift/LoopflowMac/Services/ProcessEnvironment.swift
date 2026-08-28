// Shared environment helpers for child processes spawned by the Loopflow GUI.
//
// GUI-launched apps inherit a minimal PATH (/usr/bin:/bin:/usr/sbin:/sbin) that
// doesn't include Homebrew, ~/.local/bin, or ~/.cargo/bin. Anything Loopflow
// shells out to — tmux, git, and agent CLIs — inherits that, so execvp can't
// find user-installed binaries. Pipe child envs through
// `enrichedPath` so they resolve the same binaries the user's shell would.

import Foundation

enum GUIProcessEnvironment {
    static func enrichedPath(from existing: String?) -> String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let candidates = [
            "\(home)/.local/bin",
            "/opt/homebrew/bin",
            "/opt/homebrew/sbin",
            "/usr/local/bin",
            "/usr/local/sbin",
            "\(home)/.cargo/bin",
        ]
        let existingComponents = existing?.split(separator: ":").map(String.init) ?? []
        var seen = Set(existingComponents)
        var prepended: [String] = []
        for dir in candidates where !seen.contains(dir) {
            prepended.append(dir)
            seen.insert(dir)
        }
        return (prepended + existingComponents).joined(separator: ":")
    }

    static func enriched(_ env: [String: String]) -> [String: String] {
        var copy = env
        copy["PATH"] = enrichedPath(from: env["PATH"])
        return copy
    }
}
