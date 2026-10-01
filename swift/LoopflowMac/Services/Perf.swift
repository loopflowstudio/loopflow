// Perf — os_signpost intervals at the workspace's user-visible boundaries.
//
// One signposter: subsystem "studio.loopflow", category "perf". logd records
// the intervals whether or not Instruments is attached, so `log show --signpost`
// and `xctrace` both read them; nothing leaves the machine. When no interval is
// pending the cost is one dictionary lookup and one os_signpost call.
// Interval definitions: scripts/benchmarks/desktop-performance/README.md.

import Darwin
import Foundation
import OSLog

enum Perf {
    static let signposter = OSSignposter(subsystem: "studio.loopflow", category: "perf")

    /// Process launch → first outline rows committed.
    static let coldStart: StaticString = "cold_start"
    /// Outline input (fold, presentation, filter, repository) → new rows committed.
    static let hierarchyInteraction: StaticString = "hierarchy_interaction"
    /// Wave/Task selection → detail committed; Session selection → pane accepts input.
    static let taskWorkspaceReady: StaticString = "task_workspace_ready"
    static let retainedWorkspaceAction: StaticString = "retained_workspace_action"
    /// One `lf` subprocess read; the message names the verb.
    static let lf: StaticString = "lf"
    /// One Markdown parse into blocks; the message carries the source length.
    static let markdownParse: StaticString = "markdown_parse"
    /// Terminal key press → the next surface draw after it.
    static let terminalKeyToDraw: StaticString = "terminal_key_to_draw"

    private struct Pending {
        let state: OSSignpostIntervalState
        let id: String
    }

    @MainActor private static var pending: [String: Pending] = [:]

    /// Start a user-visible interval. A still-pending interval of the same
    /// name is ended as `superseded` so the trace never shows two of them open.
    @MainActor
    static func begin(_ name: StaticString, _ scenario: String, id: String, detail: String = "") {
        let key = "\(name)"
        if let stale = pending.removeValue(forKey: key) {
            signposter.endInterval(name, stale.state, "superseded")
        }
        let state = signposter.beginInterval(
            name, id: signposter.makeSignpostID(),
            "scenario=\(scenario, privacy: .public) id=\(id, privacy: .public) \(detail, privacy: .public)"
        )
        pending[key] = Pending(state: state, id: id)
    }

    /// End the pending interval for `id`; another id or none pending is a no-op.
    @MainActor
    static func end(_ name: StaticString, id: String) {
        let key = "\(name)"
        guard let current = pending[key], current.id == id else { return }
        pending.removeValue(forKey: key)
        signposter.endInterval(name, current.state, "ready")
    }

    /// End on the next main-queue callback after the view observes the change.
    /// This is a scheduling proxy, not a render or compositor completion fence.
    @MainActor
    static func endAfterCommit(_ name: StaticString, id: String) {
        guard pending["\(name)"]?.id == id else { return }
        DispatchQueue.main.async { end(name, id: id) }
    }

    @MainActor
    static func isPending(_ name: StaticString) -> Bool {
        pending["\(name)"] != nil
    }

    static func measure<Value>(_ name: StaticString, _ detail: String, _ body: () throws -> Value) rethrows -> Value {
        let state = signposter.beginInterval(name, id: signposter.makeSignpostID(), "\(detail, privacy: .public)")
        defer { signposter.endInterval(name, state) }
        return try body()
    }

    static func measure<Value>(_ name: StaticString, _ detail: String, _ body: () async throws -> Value) async rethrows -> Value {
        let state = signposter.beginInterval(name, id: signposter.makeSignpostID(), "\(detail, privacy: .public)")
        defer { signposter.endInterval(name, state) }
        return try await body()
    }

    /// Milliseconds from kernel process start to now: the pre-main share of a
    /// cold start that no in-process interval can contain.
    static func millisecondsSinceProcessStart() -> Double? {
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var name = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
        guard sysctl(&name, UInt32(name.count), &info, &size, nil, 0) == 0 else { return nil }
        let start = info.kp_proc.p_starttime
        let started = Double(start.tv_sec) + Double(start.tv_usec) / 1_000_000
        return (Date().timeIntervalSince1970 - started) * 1000
    }
}
