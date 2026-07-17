import AppKit
import Foundation

/// The product surface a capture targets, named by `LOOPFLOW_UI_TEST_VIEW` and
/// listed in `scripts/screenshots.yaml`.
enum CaptureTarget {
    /// The always-present primary window: `wave` is it with a Wave selected,
    /// `roadmap` is it with none. Neither needs opening.
    case primary
    /// A routed window — it must be opened with the Wave the capture selects.
    case contextLab
    /// Any other Window scene id. Untyped windows are capturable without
    /// view-specific Swift, so a new site image is a manifest entry.
    case window(id: String)

    /// Titles of every window that is not the primary surface, so a capture of
    /// the primary window cannot be won by one that merely opened later.
    private static let secondaryTitles = ["Context Lab", "Portfolio", "Telemetry", "Task workspace"]

    init(view: String) {
        switch view {
        case "wave", "roadmap": self = .primary
        case "context-lab": self = .contextLab
        default: self = .window(id: view)
        }
    }

    /// A background-launched app has no key window, so the requested surface is
    /// matched by title rather than assumed.
    @MainActor
    func matches(_ window: NSWindow) -> Bool {
        switch self {
        case .primary:
            return !Self.secondaryTitles.contains(window.title)
        case .contextLab:
            return window.title == "Context Lab"
        case .window(let id):
            let expected = id.split(separator: "-").map { $0.capitalized }.joined(separator: " ")
            return window.title == expected
        }
    }
}

enum AppTestMode: String {
    case emptyWorkspaces = "empty-workspaces"
    case mockWaves = "mock-waves"
    /// Renders through the REAL `lf` registry — the same read path production
    /// uses — while keeping the snapshot/width test knobs armed. Every other
    /// mode bypasses the registry with fixture data; `live` is the honest
    /// real-Product-data leg of the surface proof, not fixture-only.
    case live = "live"

    static func current() -> AppTestMode? {
        let process = ProcessInfo.processInfo
        if let index = process.arguments.firstIndex(of: "-ui-test-mode"),
           process.arguments.count > index + 1 {
            return AppTestMode(rawValue: process.arguments[index + 1])
        }
        return process.environment["LOOPFLOW_UI_TEST_MODE"].flatMap(AppTestMode.init(rawValue:))
    }

    /// `live` runs the real registry read path (it only adds the snapshot/width
    /// test knobs); every other mode bypasses the registry with fixture data.
    /// Production (`current() == nil`) reads the registry too.
    var bypassesRegistry: Bool { self != .live }

    /// True only for deterministic fixture modes. `live` keeps the screenshot
    /// controls but must execute the same repo discovery and registry reads as
    /// production; testing `current() != nil` would silently turn it into an
    /// empty fixture surface.
    static var shouldBypassRegistry: Bool {
        current()?.bypassesRegistry == true
    }

    /// A fixed window width for a screenshot/UI-test run, from
    /// `LOOPFLOW_UI_TEST_WIDTH`. It pins the window so the narrow and wide legs
    /// of the "every Wave stays selectable without horizontal clipping" proof
    /// are deterministic — never the host's incidental default size.
    static var windowWidth: CGFloat? {
        guard let raw = ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_WIDTH"],
              let value = Double(raw), value > 0 else { return nil }
        return CGFloat(value)
    }

    /// A fixed window height for a screenshot run. Website captures set both
    /// dimensions so every image has the same frame regardless of the host's
    /// saved window state.
    static var windowHeight: CGFloat? {
        guard let raw = ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_HEIGHT"],
              let value = Double(raw), value > 0 else { return nil }
        return CGFloat(value)
    }

    /// Seconds the surface settles before the snapshot fires, from
    /// `LOOPFLOW_UI_TEST_DELAY`. The fixture legs render fast; the `live` leg
    /// shells out to `lf ls`/`lf status`, so a real-data capture gives it more
    /// room. Defaults to 2.5s — the value the fixture distinctness proof tuned.
    static var snapshotDelay: TimeInterval {
        guard let raw = ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_DELAY"],
              let value = Double(raw), value > 0 else { return 2.5 }
        return value
    }

    /// A Wave name to auto-select once the list resolves, from
    /// `LOOPFLOW_UI_TEST_SELECT_BRANCH`. The `mock-waves` fixture reads it to
    /// target a list state; `live` reads it to drive a real Wave into its full
    /// detail hierarchy headlessly — so a screenshot run captures the real
    /// objective/Projects/KRs/task rows, not just the list.
    static var selectBranch: String? {
        let env = ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_SELECT_BRANCH"]
        return (env?.isEmpty == false ? env : nil)
    }

    /// The surface a capture run targets. Older Wave-surface proofs do not set
    /// the knob, so a run with only a snapshot path still means the primary window.
    static var captureTarget: CaptureTarget? {
        let env = ProcessInfo.processInfo.environment
        if let raw = env["LOOPFLOW_UI_TEST_VIEW"], !raw.isEmpty {
            return CaptureTarget(view: raw)
        }
        return env["LOOPFLOW_UI_TEST_SNAPSHOT_PATH"] == nil ? nil : .primary
    }

    /// Website captures are deliberately light even when the laptop is dark.
    /// Ordinary UI tests and production keep the user's appearance setting.
    static var forcesLightAppearance: Bool {
        ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_APPEARANCE"] == "light"
    }
}
