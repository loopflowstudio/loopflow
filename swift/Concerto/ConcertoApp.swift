import SwiftUI
import CoreText
import LoopflowCore

extension AppearanceMode {
    static func resolvedTheme(
        rawValue: String,
        systemScheme: ColorScheme
    ) -> (preferredScheme: ColorScheme?, palette: LoopflowPalette) {
        let mode = AppearanceMode(rawValue: rawValue) ?? .system
        let palette: LoopflowPalette

        switch mode {
        case .light:
            palette = .light
        case .dark:
            palette = .dark
        case .system:
            palette = systemScheme == .dark ? .dark : .light
        }

        return (mode.colorScheme, palette)
    }
}

private enum AppFontRegistration {
    private static var fontBundle: Bundle {
        #if SWIFT_PACKAGE
        Bundle.module
        #else
        Bundle.main
        #endif
    }

    static func registerBundledFonts() {
        let fontFiles = [
            "CormorantGaramond-Regular.otf",
            "CormorantGaramond-Medium.otf",
            "CormorantGaramond-SemiBold.otf",
            "Lato-Regular.ttf",
            "Lato-Bold.ttf",
            "JetBrainsMono-Regular.ttf",
        ]

        for file in fontFiles {
            guard let url = fontBundle.url(forResource: file, withExtension: nil, subdirectory: "Fonts") else {
                continue
            }
            CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
        }
    }
}

#if os(macOS)
@main
struct ConcertoApp: App {
    @State private var portfolioService = PortfolioService()
    @State private var keyboardRouter = KeyboardRouter()
    @Environment(\.openWindow) private var openWindow
    @Environment(\.colorScheme) private var systemScheme
    @State private var snapshotError: String?
    @State private var showSnapshotError = false
    @AppStorage("appearanceMode") private var appearanceMode = AppearanceMode.system.rawValue

    init() {
        AppFontRegistration.registerBundledFonts()
        Task {
            try? await NotificationService.shared.requestAuthorization()
        }
    }

    var body: some Scene {
        let theme = AppearanceMode.resolvedTheme(rawValue: appearanceMode, systemScheme: systemScheme)
        let uiTestMode = RepoState.uiTestMode()
        let screenshotMode = RepoState.ScreenshotMode.fromArgs()

        WindowGroup {
            Group {
                if let screenshot = screenshotMode {
                    ScreenshotWindow(mode: screenshot)
                } else if uiTestMode != nil {
                    RepoWindow(
                        repoURL: URL(fileURLWithPath: "/tmp/loopflow-ui-tests"),
                        portfolioService: portfolioService
                    )
                } else {
                    PortfolioWindow(portfolioService: portfolioService)
                }
            }
            .tint(.loopflowBurgundy)
            .preferredColorScheme(theme.preferredScheme)
            .environment(\.palette, theme.palette)
            .environment(keyboardRouter)
        }
        .windowStyle(.automatic)
        .defaultSize(width: 1080, height: 760)

        WindowGroup(id: "repo", for: URL.self) { $repoURL in
            RepoWindow(repoURL: repoURL, portfolioService: portfolioService)
                .tint(.loopflowBurgundy)
                .preferredColorScheme(theme.preferredScheme)
                .environment(\.palette, theme.palette)
                .environment(keyboardRouter)
        }
        .windowStyle(.automatic)
        .defaultSize(width: 900, height: 700)

        Window("Terminal Test", id: "terminal-test") {
            TerminalTestWindow()
                .preferredColorScheme(theme.preferredScheme)
                .environment(\.palette, theme.palette)
        }
        .defaultSize(width: 800, height: 600)

        .commands {
            CommandGroup(after: .appSettings) {
                Toggle("Beta Features", isOn: Binding(
                    get: { Flags.beta },
                    set: { Flags.setBeta($0) }
                ))
                Divider()
                Picker("Appearance", selection: Binding(
                    get: { appearanceMode },
                    set: { appearanceMode = $0 }
                )) {
                    ForEach(AppearanceMode.allCases, id: \.rawValue) { mode in
                        Text(mode.menuTitle).tag(mode.rawValue)
                    }
                }
                .pickerStyle(.radioGroup)
            }

            CommandGroup(after: .saveItem) {
                Button("Snapshot for Review") {
                    snapshotCurrentWindow()
                }
                .keyboardShortcut("4", modifiers: [.command])
            }

            CommandGroup(after: .sidebar) {
                Button("Command Palette") {
                    NotificationCenter.default.post(name: .toggleCommandPalette, object: nil)
                }
                .keyboardShortcut("k", modifiers: .command)

                Divider()

                Button("Terminal Test") {
                    openWindow(id: "terminal-test")
                }
                .keyboardShortcut("t", modifiers: [.command, .shift])
            }
        }
    }

    @MainActor
    private func snapshotCurrentWindow() {
        let snapshotService = SnapshotService()

        do {
            let outputURL = try snapshotService.snapshotKeyWindow()
            NSSound.beep()
            NSWorkspace.shared.activateFileViewerSelecting([outputURL])
        } catch {
            snapshotError = error.localizedDescription
            showSnapshotError = true
        }
    }
}
#else
@main
struct ConcertoApp: App {
    init() {
        AppFontRegistration.registerBundledFonts()
        Task {
            try? await NotificationService.shared.requestAuthorization()
        }
    }

    var body: some Scene {
        WindowGroup {
            MobileRootView()
        }
    }
}
#endif
