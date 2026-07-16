import Foundation

enum AppTestMode: String {
    case emptyWorkspaces = "empty-workspaces"
    case sampleWorkspaces = "sample-workspaces"
    case mockWaves = "mock-waves"

    static func current() -> AppTestMode? {
        let process = ProcessInfo.processInfo
        if let index = process.arguments.firstIndex(of: "-ui-test-mode"),
           process.arguments.count > index + 1 {
            return AppTestMode(rawValue: process.arguments[index + 1])
        }
        return process.environment["LOOPFLOW_UI_TEST_MODE"].flatMap(AppTestMode.init(rawValue:))
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
}
