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
    static func registerBundledFonts() {
        guard let fontsDir = findFontsDirectory() else { return }

        let fontFiles = [
            "CormorantGaramond-Regular.otf",
            "CormorantGaramond-Medium.otf",
            "CormorantGaramond-SemiBold.otf",
            "Lato-Regular.ttf",
            "Lato-Bold.ttf",
            "JetBrainsMono-Regular.ttf",
        ]

        for file in fontFiles {
            let url = fontsDir.appendingPathComponent(file)
            guard FileManager.default.fileExists(atPath: url.path) else { continue }
            CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
        }
    }

    private static func findFontsDirectory() -> URL? {
        let fm = FileManager.default

        // SPM resource bundle in Contents/Resources/ (release .app)
        if let resourceURL = Bundle.main.resourceURL {
            let spmBundle = resourceURL
                .appendingPathComponent("LoopflowSwift_Concerto.bundle")
                .appendingPathComponent("Fonts")
            if fm.fileExists(atPath: spmBundle.path) {
                return spmBundle
            }
        }

        // SPM resource bundle adjacent to executable (dev builds via `swift run`)
        let executableBundle = Bundle.main.bundleURL
            .appendingPathComponent("LoopflowSwift_Concerto.bundle")
            .appendingPathComponent("Fonts")
        if fm.fileExists(atPath: executableBundle.path) {
            return executableBundle
        }

        // Fonts directly in Resources/ (xcodegen builds)
        if let resourceURL = Bundle.main.resourceURL {
            let direct = resourceURL.appendingPathComponent("Fonts")
            if fm.fileExists(atPath: direct.path) {
                return direct
            }
        }

        return nil
    }
}

@MainActor
private enum VoiceInputWarmup {
    static let service = VoiceInputService()
    private static var hasStarted = false

    static func start() {
        guard !hasStarted else { return }
        hasStarted = true
        service.prewarmModelInBackground()
    }
}

private enum AppRuntime {
    static var isAutomatedTest: Bool {
        let processInfo = ProcessInfo.processInfo
        let environment = processInfo.environment
        if environment["XCTestConfigurationFilePath"] != nil ||
            environment["CONCERTO_UI_TEST_MODE"] != nil {
            return true
        }

        let arguments = processInfo.arguments
        return arguments.contains("-ui-test-mode") || arguments.contains("--snapshot")
    }
}

private enum LaunchArguments {
    static func repoURL() -> URL? {
        let args = ProcessInfo.processInfo.arguments
        guard let index = args.firstIndex(of: "--repo"), args.count > index + 1 else {
            return nil
        }
        return URL(fileURLWithPath: args[index + 1])
    }
}

private func bootstrapConcertoApp() {
    AppFontRegistration.registerBundledFonts()
    guard !AppRuntime.isAutomatedTest else { return }
    Task {
        try? await NotificationService.shared.requestAuthorization()
    }
    Task { @MainActor in
        VoiceInputWarmup.start()
    }
    #if os(macOS)
    Task { @MainActor in
        SharedDaemon.eagerStart()
    }
    #endif
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
        bootstrapConcertoApp()
    }

    var body: some Scene {
        let theme = AppearanceMode.resolvedTheme(rawValue: appearanceMode, systemScheme: systemScheme)
        let uiTestMode = RepoState.uiTestMode()
        let screenshotMode = RepoState.ScreenshotMode.fromArgs()
        let launchRepoURL = screenshotMode == nil ? LaunchArguments.repoURL() : nil

        WindowGroup {
            Group {
                if let screenshot = screenshotMode {
                    ScreenshotWindow(mode: screenshot)
                } else if uiTestMode != nil {
                    RepoWindow(
                        repoURL: URL(fileURLWithPath: "/tmp/loopflow-ui-tests"),
                        portfolioService: portfolioService
                    )
                } else if let launchRepoURL {
                    RepoWindow(repoURL: launchRepoURL, portfolioService: portfolioService)
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

        Window("Reply Demo", id: "reply-demo") {
            ReplyDemoView()
                .preferredColorScheme(theme.preferredScheme)
                .environment(\.palette, theme.palette)
        }
        .defaultSize(width: 1200, height: 760)

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
            }

            CommandMenu("Debug") {
                Button("Terminal Test") {
                    openWindow(id: "terminal-test")
                }
                .keyboardShortcut("t", modifiers: [.command, .shift])

                Button("Reply Demo") {
                    openWindow(id: "reply-demo")
                }
                .keyboardShortcut("r", modifiers: [.command, .shift])
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
        bootstrapConcertoApp()
    }

    var body: some Scene {
        WindowGroup {
            MobileRootView()
        }
    }
}
#endif
