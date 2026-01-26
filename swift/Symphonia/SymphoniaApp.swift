// Loopflow Symphonia - Teams product.
// macOS 15+ native app for team coordination of LLM coding waves.

import SwiftUI
import LoopflowCore

@main
struct SymphoniaApp: App {
    var body: some Scene {
        WindowGroup {
            PlaceholderView()
        }
        .windowStyle(.automatic)
        .defaultSize(width: 900, height: 700)
    }
}
