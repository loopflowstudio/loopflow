import SwiftUI
import AppKit
import Loopflow

private func enrichProcessPathForGUILaunch() {
    let existing = ProcessInfo.processInfo.environment
    let enriched = GUIProcessEnvironment.enriched(existing)
    for key in existing.keys where enriched[key] == nil { unsetenv(key) }
    if let path = enriched["PATH"] { setenv("PATH", path, 1) }
}

@main
struct LoopflowApp: App {
    @State private var taskLinks = WorkLinkRouter()
    @State private var portfolioService = PortfolioService()
    @Environment(\.openWindow) private var openWindow
    @State private var snapshotError: String?
    @State private var showSnapshotError = false
    @State private var openingError: String?
    @State private var taskChoices: [[TaskWindowChoice]] = []
    @State private var didOpenCaptureView = false
    @AppStorage("taskFilesAutosave") private var taskFilesAutosave = true
    @AppStorage("appearanceMode") private var appearanceMode = AppearanceMode.system.rawValue

    init() {
        NSWindow.allowsAutomaticWindowTabbing = false
        Perf.begin(Perf.coldStart, "launch", id: "launch",
                   detail: "pre_main_ms=\(Int(Perf.millisecondsSinceProcessStart() ?? -1))")
        Perf.begin(Perf.workspaceCurrent, "launch", id: "launch")
        if AppTestMode.current() == nil { LaunchJournal.home.begin() }
        bootstrapLoopflowApp()
        // Enrich our own process PATH before any children spawn, so tools launched
        // by Wave launchers can find git and agent CLIs that live in
        // Homebrew or ~/.local/bin.
        enrichProcessPathForGUILaunch()
    }

    var body: some Scene {
        let resolvedAppearance = AppTestMode.forcesLightAppearance
            ? AppearanceMode.light.rawValue
            : appearanceMode
        let launchRepoURL = LaunchArguments.repoURL()
        let registryQuery = SessionFixture.query ?? RegistryQueryLocal.shared

        WindowGroup(id: "workspace", for: RepositoryWorkspace.self) { $workspace in
            Group {
                if let workspace {
                    RepositoryWorkspaceView(workspace: workspace, portfolioService: portfolioService,
                                            query: registryQuery, router: taskLinks,
                                            openRepository: requestRepository)
                } else if AppTestMode.shouldBypassRegistry {
                    RepoView(portfolioService: portfolioService, initialRepoPath: launchRepoURL?.path,
                             query: registryQuery)
                } else {
                    WorkspaceLaunchView {
                        if let path = launchRepoURL?.path ?? loadLoopflowState()?.selectedRepoPath {
                            try await openRepository(path)
                        } else {
                            openWindow(id: "portfolio")
                        }
                    }
                }
            }
            .tint(.loopflowBurgundy)
            .modifier(AppAppearance(mode: resolvedAppearance))
            .onOpenURL { handleDeepLink($0) }
            .uiTestWindowWidth()
            .uiTestSnapshot()
            .task { openCaptureViewIfNeeded() }
        }
        .windowStyle(.automatic)
        .defaultSize(width: 1280, height: 800)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("Open Repo…") { openRepoPanel() }
                    .keyboardShortcut("n", modifiers: .command)
            }
            CommandGroup(after: .appSettings) {
                Toggle("Autosave Task Files", isOn: $taskFilesAutosave)
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
                                requestRepository(repo.path, nil)
                            }
                        }
                    }
                }

                Button("Open Repo…") {
                    openRepoPanel()
                }
                .keyboardShortcut("o", modifiers: [.command, .shift])
            }
        }

        Window("Portfolio", id: "portfolio") {
            WavesView(portfolioService: portfolioService, openTask: { path, id in
                var link = URLComponents()
                link.scheme = "loopflow"; link.host = "task"; link.path = "/" + id
                link.queryItems = [URLQueryItem(name: "repo", value: path)]
                requestRepository(path, link.url)
            })
                .tint(.loopflowBurgundy)
                .modifier(AppAppearance(mode: resolvedAppearance))
                .onOpenURL { handleDeepLink($0) }
        }
        .defaultSize(width: 1080, height: 760)

        Window("Open Work", id: "open-work") {
            WorkspaceOpeningFeedback(error: $openingError, choices: $taskChoices,
                                     openRepository: requestRepository)
                .modifier(AppAppearance(mode: resolvedAppearance))
        }
        .defaultSize(width: 480, height: 240)

        Window("Telemetry", id: "telemetry") {
            TelemetryDashboardView()
                .tint(.loopflowBurgundy)
                .modifier(AppAppearance(mode: resolvedAppearance))
                .onOpenURL { handleDeepLink($0) }
        }
        .defaultSize(width: 1180, height: 860)

    }

    @MainActor
    private func openCaptureViewIfNeeded() {
        guard !didOpenCaptureView, let target = AppTestMode.captureTarget else { return }
        didOpenCaptureView = true
        switch target {
        case .primary:
            break
        case .window(let id):
            openWindow(id: id)
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
        case "task":
            Task {
                do {
                    let link = try TaskLink(url: url)
                    if let path = link.repo {
                        try await openRepository(path, link: url)
                        return
                    }
                    let query = SessionFixture.query ?? RegistryQueryLocal.shared
                    let snapshot = try await query.taskDestination(issue: link.issue, repo: nil)
                    let choices = snapshot.waves.flatMap { wave in
                        wave.tasks.items.map { _ in TaskWindowChoice(path: wave.wave.repo, url: url) }
                    }
                    guard !snapshot.waves.contains(where: { $0.tasks.unavailableReason != nil }) else {
                        throw RegistryQueryError("Task lookup is incomplete. Retry, or qualify the link with ?repo=/path/to/repository.")
                    }
                    if choices.count == 1, let choice = choices.first {
                        try await openRepository(choice.path, link: choice.url)
                    } else if choices.isEmpty {
                        throw RegistryQueryError("Task \(link.issue) was not found. No workspace was changed.")
                    } else {
                        taskChoices.append(choices)
                        openWindow(id: "open-work")
                    }
                } catch { reportOpeningError(error) }
            }
        case "open":
            guard let repoPath = URLComponents(url: url, resolvingAgainstBaseURL: false)?
                .queryItems?.first(where: { $0.name == "repo" })?.value
            else { return }
            requestRepository(repoPath, nil)
        case "portfolio":
            openWindow(id: "portfolio")
        case "sessions":
            NotificationCenter.default.post(name: .openSessions, object: nil)
        default:
            break
        }
    }

    @MainActor
    private func reportOpeningError(_ error: Error) {
        openingError = error.localizedDescription
        openWindow(id: "open-work")
    }

    @MainActor
    private func requestRepository(_ path: String, _ link: URL?) {
        Task {
            do { try await openRepository(path, link: link) }
            catch { reportOpeningError(error) }
        }
    }

    @MainActor
    private func openRepository(_ path: String, link: URL? = nil) async throws {
        let query = SessionFixture.query ?? RegistryQueryLocal.shared
        let workspace = try await RepositoryWorkspace.resolve(path: path, query: query)
        portfolioService.addRepo(URL(fileURLWithPath: workspace.path))
        var destination = link
        if let link, var components = URLComponents(url: link, resolvingAgainstBaseURL: false) {
            components.queryItems = (components.queryItems ?? []).filter { $0.name != "repo" }
                + [URLQueryItem(name: "repo", value: workspace.path)]
            destination = components.url
        }
        if !taskLinks.deliver(destination, repository: workspace.id) {
            openWindow(id: "workspace", value: workspace)
        }
        try? saveLoopflowState(LoopflowState(selectedRepoPath: workspace.path))
    }

    @MainActor
    private func openRepoPanel() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.prompt = "Open Repo"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        guard let mainRepo = portfolioService.addRepo(url) else {
            let alert = NSAlert()
            alert.messageText = "“\(url.lastPathComponent)” isn’t a Git repository"
            alert.informativeText =
                "Loopflow works inside a project that uses Git. "
                + "Choose a folder that already does, or run “git init” in this one first."
            alert.runModal()
            return
        }
        requestRepository(mainRepo.path, nil)
    }
}

/// Resolve system appearance inside the window, where SwiftUI supplies its
/// effective color scheme. An App-level environment read cannot supply it.
struct AppAppearance: ViewModifier {
    let mode: String
    @Environment(\.colorScheme) private var systemScheme

    func body(content: Content) -> some View {
        let theme = AppearanceMode.resolvedTheme(rawValue: mode, systemScheme: systemScheme)
        content
            .preferredColorScheme(theme.preferredScheme)
            .environment(\.colorScheme, theme.preferredScheme ?? systemScheme)
            .environment(\.palette, theme.palette)
    }
}

private extension View {
    /// Pin the window to `LOOPFLOW_UI_TEST_WIDTH` when a UI-test run renders
    /// without a snapshot path — the narrow and wide legs of the
    /// selectable-without-clipping proof. Snapshot runs skip this pin:
    /// `uiTestSnapshot()` sizes the real window at capture time, width and
    /// height together.
    @ViewBuilder
    func uiTestWindowWidth() -> some View {
        if let width = AppTestMode.viewPinnedWidth {
            frame(width: width)
                .frame(maxHeight: .infinity, alignment: .top)
        } else {
            self
        }
    }

    /// In a UI-test run, once the surface has settled, render the key window to
    /// a PNG at `LOOPFLOW_UI_TEST_SNAPSHOT_PATH` and exit. `SnapshotService`
    /// renders the view (`cacheDisplay`) rather than the screen, so this needs
    /// no Screen Recording or Automation permission — it is the run-here leg of
    /// the state-distinctness proof (`scripts/prove_wave_surface_states.sh`),
    /// complementing the permissioned XCUITest.
    @ViewBuilder
    func uiTestSnapshot() -> some View {
        if AppTestMode.current() != nil,
           let target = AppTestMode.captureTarget,
           let path = ProcessInfo.processInfo.environment["LOOPFLOW_UI_TEST_SNAPSHOT_PATH"] {
            let delay = AppTestMode.snapshotDelay
            task {
                var captureWindow: NSWindow?
                for _ in 0 ..< 50 {
                    captureWindow = NSApp.windows.first { window in
                        window.isVisible && window.contentView != nil && target.matches(window)
                    }
                    if captureWindow != nil { break }
                    try? await Task.sleep(nanoseconds: 100_000_000)
                }

                if let window = captureWindow {
                    if let width = AppTestMode.windowWidth {
                        let height = AppTestMode.windowHeight
                            ?? window.contentView?.frame.height
                            ?? window.frame.height
                        window.setContentSize(NSSize(width: width, height: height))
                    }
                    try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
                    if window.isVisible {
                        do {
                            _ = try SnapshotService().snapshotWindow(window, to: path)
                        } catch {
                            fputs("website capture failed: \(error)\n", stderr)
                        }
                    } else {
                        fputs("website capture failed: target window closed before capture\n", stderr)
                    }
                } else {
                    fputs("website capture failed: target window did not open\n", stderr)
                }
                NSApp.terminate(nil)
            }
        } else {
            self
        }
    }
}
