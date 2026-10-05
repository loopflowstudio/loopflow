// Shared environment helpers for child processes spawned by the Loopflow GUI.
//
// GUI-launched apps inherit a minimal PATH (/usr/bin:/bin:/usr/sbin:/sbin) that
// doesn't include Homebrew, ~/.local/bin, or ~/.cargo/bin. Anything Loopflow
// shells out to — git and the bundled lf — inherits that, so execvp can't
// find user-installed binaries. Pipe child envs through `enrichedPath` so they
// resolve the same binaries the user's shell would.

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
        // The GUI is a new human control surface, not a continuation of the
        // Work process that opened it. Preserve Home and account authority.
        for key in ["LF_WAVE_ID", "LF_RUN_ID", "LF_RUN_DIR", "LF_TRACE_ID", "LF_PROCESS_ID",
                    "LF_AS", "LF_FLOW_STEP", "LF_HUMAN_SESSION", "LF_AGENT_CALLER",
                    "LF_GIT_OPERATION_ID", "LF_WORK_ADVANCE_CLAIM", "LOOPFLOW_DIRECTIVE_FILE",
                    "LF_TERMINAL_ID", "LF_TERMINAL_TTY"] {
            copy.removeValue(forKey: key)
        }
        // Embedded terminals are their own terminal. A launcher's terminal
        // session and agent output policy (`open` passes both through) would
        // otherwise strip color and paging from every pane. CODEX_HOME is
        // account authority, not output policy.
        for key in copy.keys where key != "CODEX_HOME"
            && (launcherTerminalKeys.contains(key)
                || launcherTerminalPrefixes.contains(where: key.hasPrefix)) {
            copy.removeValue(forKey: key)
        }
        return copy
    }

    private static let launcherTerminalKeys: Set<String> = [
        "NO_COLOR", "FORCE_COLOR", "CLICOLOR", "CLICOLOR_FORCE", "COLORTERM",
        "TERM", "TERM_PROGRAM", "TERM_PROGRAM_VERSION", "TMUX", "TMUX_PANE",
        "CI", "PAGER", "GIT_PAGER", "GH_PAGER", "AI_AGENT", "CLAUDECODE",
    ]
    private static let launcherTerminalPrefixes = ["WARP_", "CLAUDE_CODE_", "CODEX_"]
}
