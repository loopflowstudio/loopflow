import SwiftUI
import AppKit
import Loopflow

private func enrichProcessPathForGUILaunch() {
    let existing = ProcessInfo.processInfo.environment["PATH"]
    let enriched = GUIProcessEnvironment.enrichedPath(from: existing)
    guard enriched != existing else { return }
    setenv("PATH", enriched, 1)
}

@main
struct LoopflowApp: App {
    @State private var portfolioService = PortfolioService()
    @Environment(\.openWindow) private var openWindow
    @Environment(\.colorScheme) private var systemScheme
    @State private var snapshotError: String?
    @State private var showSnapshotError = false
    @AppStorage("appearanceMode") private var appearanceMode = AppearanceMode.system.rawValue

    init() {
        NSWindow.allowsAutomaticWindowTabbing = false
        bootstrapLoopflowApp()
        // Enrich our own process PATH before any children spawn, so tools launched
        // through Ghostty surfaces and `--noprofile --norc` shells can find tmux,
        // git, and agent CLIs that live in Homebrew or ~/.local/bin.
        enrichProcessPathForGUILaunch()
    }

    var body: some Scene {
        let theme = AppearanceMode.resolvedTheme(rawValue: appearanceMode, systemScheme: systemScheme)
        let launchRepoURL = LaunchArguments.repoURL()

        WindowGroup {
            WavesView(
                portfolioService: portfolioService,
                initialRepoPath: launchRepoURL?.path
            )
            .tint(.loopflowBurgundy)
            .preferredColorScheme(theme.preferredScheme)
            .environment(\.palette, theme.palette)
            .onOpenURL { handleDeepLink($0) }
        }
        .windowStyle(.automatic)
        .defaultSize(width: 1080, height: 760)

        WindowGroup(id: "repo", for: URL.self) { $repoURL in
            WavesView(
                portfolioService: portfolioService,
                initialRepoPath: repoURL?.path
            )
            .tint(.loopflowBurgundy)
            .preferredColorScheme(theme.preferredScheme)
            .environment(\.palette, theme.palette)
        }
        .windowStyle(.automatic)
        .defaultSize(width: 1080, height: 760)

        Window("Portfolio", id: "portfolio") {
            WavesView(portfolioService: portfolioService)
                .tint(.loopflowBurgundy)
                .preferredColorScheme(theme.preferredScheme)
                .environment(\.palette, theme.palette)
        }
        .defaultSize(width: 1080, height: 760)

        Window("Telemetry", id: "telemetry") {
            TelemetryDashboardView()
                .tint(.loopflowBurgundy)
                .preferredColorScheme(theme.preferredScheme)
                .environment(\.palette, theme.palette)
        }
        .defaultSize(width: 1180, height: 860)

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

            CommandMenu("Go") {
                Button("Portfolio") {
                    openWindow(id: "portfolio")
                }
                .keyboardShortcut("0", modifiers: .command)

                Button("Telemetry") {
                    openWindow(id: "telemetry")
                }
                .keyboardShortcut("1", modifiers: .command)

                if !portfolioService.repos.isEmpty {
                    Menu("Move to Repo") {
                        ForEach(portfolioService.repos) { repo in
                            Button(repo.displayName) {
                                portfolioService.addRepo(repo.url)
                                openWindow(id: "repo", value: repo.url)
                            }
                        }
                    }
                }

                Button("Open Repo…") {
                    openRepoPanel()
                }
                .keyboardShortcut("o", modifiers: [.command, .shift])
            }

            CommandMenu("Debug") {
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

    @MainActor
    private func handleDeepLink(_ url: URL) {
        guard url.scheme == "loopflow" else { return }
        switch url.host {
        case "open":
            guard let repoPath = URLComponents(url: url, resolvingAgainstBaseURL: false)?
                .queryItems?.first(where: { $0.name == "repo" })?.value
            else { return }
            let repoURL = URL(fileURLWithPath: repoPath)
            portfolioService.addRepo(repoURL)
            openWindow(id: "repo", value: repoURL)
        case "portfolio":
            openWindow(id: "portfolio")
        default:
            break
        }
    }

    @MainActor
    private func openRepoPanel() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.prompt = "Open Repo"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        portfolioService.addRepo(url)
        openWindow(id: "repo", value: url)
    }
}
