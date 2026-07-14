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
}
