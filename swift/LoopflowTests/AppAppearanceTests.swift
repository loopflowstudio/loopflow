#if os(macOS)
import AppKit
import SwiftUI
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Headless window appearance", .serialized)
@MainActor
struct AppAppearanceTests {
    @Test("The window modifier follows preview, Cancel and Apply", arguments: ThemeFixture.allCases)
    func windowModifier(theme: ThemeFixture) throws {
        let suite = "window-theme-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let appearance = AppearanceStore(defaults: defaults, systemDark: false)
        let appliedID = theme == .dark ? "light" : "dark"
        appearance.applyMode(AppearanceMode(rawValue: appliedID)!)
        let root = Text("Work").modifier(AppAppearance(appearance: appearance))
        func inspectAppearance() throws {
            let view = try root.inspect()
            let theme = try view.environment(\.theme)
            #expect(theme.id == appearance.effectiveTheme.id)
            #expect(theme.terminal == appearance.effectiveTheme.terminal)
            #expect(try view.environment(\.palette).text == theme.palette.text)
            #expect(try view.environment(\.palette).surface == theme.palette.surface)
            #expect(try view.environment(\.colorScheme) == theme.colorScheme)
            #expect(try view.preferredColorScheme() == theme.colorScheme)
        }
        try theme.preview(in: appearance)
        try inspectAppearance()
        appearance.cancel()
        #expect(appearance.effectiveTheme.id == appliedID)
        try inspectAppearance()
        try theme.preview(in: appearance)
        appearance.apply()
        try inspectAppearance()
        #expect(AppearanceStore(defaults: defaults, systemDark: true).effectiveTheme.id == appearance.effectiveTheme.id)
    }

    @Test("System light/dark changes reach the window integration", arguments: [false, true])
    func systemAppearance(dark: Bool) throws {
        let suite = UUID().uuidString
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let appearance = AppearanceStore(defaults: defaults, systemDark: !dark)
        appearance.setSystemDark(dark)
        let root = Text("Work").modifier(AppAppearance(appearance: appearance))
        #expect(try root.inspect().environment(\.colorScheme) == (dark ? .dark : .light))
        #expect(try root.inspect().environment(\.theme).id == (dark ? "dark" : "light"))
    }
}

@Suite("Native window appearance", .requiresDisplay, .serialized)
@MainActor
struct NativeAppAppearanceTests {
    @Test("Native controls and the custom palette follow the same window appearance",
          arguments: ["system", "light", "dark"], [ColorScheme.light, .dark])
    func paletteFollowsWindow(mode: String, system: ColorScheme) async throws {
        _ = NSApplication.shared
        var observed: (ColorScheme, Color)?
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let appearance = AppearanceStore(defaults: defaults, systemDark: system == .dark)
        appearance.applyMode(AppearanceMode(rawValue: mode)!)
        let view = AppearanceProbe { observed = ($0, $1) }
            .modifier(AppAppearance(appearance: appearance))
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
