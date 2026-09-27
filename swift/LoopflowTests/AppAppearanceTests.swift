#if os(macOS)
import AppKit
import SwiftUI
import Testing
@testable import Loopflow
@testable import LoopflowMac

@Suite("Window appearance", .serialized)
@MainActor
struct AppAppearanceTests {
    @Test("Native controls and the custom palette follow the same window appearance",
          arguments: ["system", "light", "dark"], [ColorScheme.light, .dark])
    func paletteFollowsWindow(mode: String, system: ColorScheme) async throws {
        _ = NSApplication.shared
        var observed: (ColorScheme, Color)?
        let view = AppearanceProbe { observed = ($0, $1) }
            .modifier(AppAppearance(mode: mode))
            .environment(\.colorScheme, system)
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 300, height: 150),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: view)
        defer { window.contentView = nil }
        window.layoutIfNeeded()
        let deadline = ContinuousClock.now + .seconds(2)
        while observed == nil, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(20))
        }
        let (scheme, text) = try #require(observed)
        let expected = AppearanceMode(rawValue: mode)?.colorScheme ?? system
        #expect(scheme == expected)
        #expect(text == (expected == .dark ? LoopflowPalette.dark.text : LoopflowPalette.light.text))
    }
}

private struct AppearanceProbe: View {
    @Environment(\.colorScheme) private var scheme
    @Environment(\.palette) private var palette
    let observe: (ColorScheme, Color) -> Void

    var body: some View {
        let _ = observe(scheme, palette.text)
        Text("Work").foregroundStyle(palette.text).background(palette.surface)
    }
}
#endif
