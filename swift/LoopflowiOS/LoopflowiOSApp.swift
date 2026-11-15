import Loopflow
import SwiftUI

@main
struct LoopflowiOSApp: App {
    init() { bootstrapLoopflowApp() }

    var body: some Scene {
        WindowGroup { MachineConnectionView() }
    }
}
