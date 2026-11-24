#if os(macOS)
import AppKit
import SwiftUI
import Synchronization
import Testing
import ViewInspector
@testable import Loopflow
@testable import LoopflowMac

@Suite("Headless window appearance", .serialized)
@MainActor
struct AppAppearanceTests {
    @Test("The window modifier follows preview, Cancel and Apply", arguments: ThemeFixture.allCases)
    func windowModifier(theme: ThemeFixture) async throws {
        try await withThemeServer { location, paths in
            let appearance = try await AppearanceStore.loaded(paths: paths, systemDark: false, location: location)
            let applied: AppearanceMode = theme == .dark ? .light : .dark
            await appearance.applyMode(applied)
            let root = Text("Work").modifier(AppAppearance(appearance: appearance))
            @MainActor func inspectAppearance() throws {
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
            #expect(appearance.effectiveTheme.isDark == (applied == .dark))
            try inspectAppearance()
            try theme.preview(in: appearance)
            await appearance.apply()
            try inspectAppearance()
            let restored = try await AppearanceStore.loaded(paths: paths, systemDark: true, location: location)
            #expect(restored.effectiveTheme.id == appearance.effectiveTheme.id)
        }
    }

    @Test("A failed theme read keeps the last good theme and offers no Choose Theme…")
    func failedRead() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("theme-server-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let lf = try BuiltLf(home: root)
        _ = try await lf.run(["desktop", "portfolio", "--json"])
        let server = LocalServerFixture(lf)
        let gone = Mutex(false)
        let location = ServerLocation(startsServer: true) { _ in
            if gone.withLock({ $0 }) { throw RegistryQueryError("Loopflow's server isn't running.") }
            return try await server.status()
        }
        let appearance = try await AppearanceStore.loaded(paths: .temporary(root: root), systemDark: false, location: location)
        await appearance.applyMode(.dark)
        gone.withLock { $0 = true }
        await server.stop()
        await appearance.reload()
        #expect(appearance.effectiveTheme.name == "Loopflow Dark")
        #expect(appearance.notice == nil)
        let failure = try #require(appearance.readFailure)
        let window = Text("Work").modifier(AppAppearance(appearance: appearance))
        #expect(try window.inspect().find(text: failure).string() == failure)
        #expect(throws: (any Error).self) { try window.inspect().find(button: "Choose Theme…") }
    }

    @Test("System light/dark changes reach the window integration", arguments: [false, true])
    func systemAppearance(dark: Bool) throws {
        let appearance = AppearanceStore(paths: .temporary(), systemDark: !dark)
        appearance.setSystemDark(dark)
        let root = Text("Work").modifier(AppAppearance(appearance: appearance))
        #expect(try root.inspect().environment(\.colorScheme) == (dark ? .dark : .light))
        #expect(try root.inspect().environment(\.theme).name == (dark ? "Loopflow Dark" : "Loopflow Light"))
    }
}

@Suite("Native window appearance", .requiresDisplay, .serialized)
@MainActor
struct NativeAppAppearanceTests {
    @Test("Native controls and the custom palette follow the same window appearance",
          arguments: ["system", "light", "dark"], [ColorScheme.light, .dark])
    func paletteFollowsWindow(mode: String, system: ColorScheme) async throws {
        try await withThemeServer { location, _ in
            _ = NSApplication.shared
            var observed: (ColorScheme, Color)?
            let appearance = try await AppearanceStore.loaded(paths: .temporary(), systemDark: system == .dark, location: location)
            await appearance.applyMode(AppearanceMode(rawValue: mode)!)
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
