import Foundation
import Testing

extension Trait where Self == ConditionTrait {
    static var requiresDisplay: Self {
        .enabled(if: ProcessInfo.processInfo.environment["LOOPFLOW_NATIVE_TESTS"] == "1",
                 "Optional display-session diagnostic")
    }
}
