import Foundation

enum AppTestMode: String {
    case emptyWorkspaces = "empty-workspaces"
    case sampleWorkspaces = "sample-workspaces"
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
}
