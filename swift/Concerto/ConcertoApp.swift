import SwiftUI
import CoreText
#if os(macOS)
import AppKit
#endif
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

@MainActor
private enum LaunchArguments {
    private static var didConsumeRepo = false

    static func repoURL() -> URL? {
        let args = ProcessInfo.processInfo.arguments
        guard let index = args.firstIndex(of: "--repo"), args.count > index + 1 else {
            return nil
        }
        return URL(fileURLWithPath: args[index + 1])
    }

    // Returns the launch repo URL only on the first call. Subsequent calls
    // return nil so additional windows (Cmd-N) fall through to the picker
    // instead of re-opening the launch repo.
    static func consumeRepoURL() -> URL? {
        guard !didConsumeRepo, let url = repoURL() else { return nil }
        didConsumeRepo = true
        return url
    }
}

private func bootstrapConcertoApp() {
    AppFontRegistration.registerBundledFonts()
    #if os(macOS)
    // Enrich our own process PATH before any children spawn, so tools launched
    // through Ghostty surfaces and `--noprofile --norc` shells can find tmux,
    // git, and agent CLIs that live in Homebrew or ~/.local/bin.
    enrichProcessPathForGUILaunch()
    #endif
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
private func enrichProcessPathForGUILaunch() {
    let existing = ProcessInfo.processInfo.environment["PATH"]
    let enriched = GUIProcessEnvironment.enrichedPath(from: existing)
    guard enriched != existing else { return }
    setenv("PATH", enriched, 1)
}
#endif

#if os(macOS)
@MainActor
private enum TmuxTerminationCleanup {
    private static var observer: NSObjectProtocol?

    static func install() {
        guard observer == nil else { return }
        observer = NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification,
            object: nil,
            queue: nil
        ) { _ in
            TmuxSessionRegistry.shared.killAllSynchronously()
        }
    }
}
#endif

#if os(macOS)
private struct AppRootContent: View {
    let portfolioService: PortfolioService
    let screenshotMode: RepoState.ScreenshotMode?
    let isUITestMode: Bool

    @State private var resolvedRepoURL: URL?

    init(
        portfolioService: PortfolioService,
        screenshotMode: RepoState.ScreenshotMode?,
        isUITestMode: Bool
    ) {
        self.portfolioService = portfolioService
        self.screenshotMode = screenshotMode
        self.isUITestMode = isUITestMode
        // Consume the launch arg once per process. The first window grabs
        // it; later windows (Cmd-N) get nil and fall through to the picker.
        let initial: URL? = (screenshotMode == nil && !isUITestMode)
            ? LaunchArguments.consumeRepoURL()
            : nil
        _resolvedRepoURL = State(initialValue: initial)
    }

    var body: some View {
        if let mode = screenshotMode {
            ScreenshotWindow(mode: mode)
        } else if isUITestMode {
            RepoWindow(
                repoURL: URL(fileURLWithPath: "/tmp/loopflow-ui-tests"),
                portfolioService: portfolioService
            )
        } else if let url = resolvedRepoURL {
            RepoWindow(repoURL: url, portfolioService: portfolioService)
        } else {
            PortfolioWindow(portfolioService: portfolioService)
        }
    }
}

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
        NSWindow.allowsAutomaticWindowTabbing = false
        bootstrapConcertoApp()
        TmuxTerminationCleanup.install()
    }

    var body: some Scene {
        let theme = AppearanceMode.resolvedTheme(rawValue: appearanceMode, systemScheme: systemScheme)
        let uiTestMode = RepoState.uiTestMode()
        let screenshotMode = RepoState.ScreenshotMode.fromArgs()

        WindowGroup {
            AppRootContent(
                portfolioService: portfolioService,
                screenshotMode: screenshotMode,
                isUITestMode: uiTestMode != nil
            )
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
