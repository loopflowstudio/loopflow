import SwiftUI
import Loopflow

@main
struct LoopflowApp: App {
    init() {
        bootstrapLoopflowApp()
    }

    var body: some Scene {
        WindowGroup {
            MobileRootView()
        }
    }
}
