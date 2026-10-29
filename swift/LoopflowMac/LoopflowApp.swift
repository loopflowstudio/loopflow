import SwiftUI
import AppKit
import Loopflow

private func enrichProcessPathForGUILaunch() {
    let existing = ProcessInfo.processInfo.environment
    let enriched = GUIProcessEnvironment.enriched(existing)
    for key in existing.keys where enriched[key] == nil { unsetenv(key) }
    for key in ["PATH", "LF_BIN", "LF_DESKTOP"] {
        if let value = enriched[key] { setenv(key, value, 1) }
    }
}

@main
struct LoopflowApp: App {
    @State private var taskLinks = WorkLinkRouter.shared
    @State private var portfolioService = PortfolioService()
    @State private var workspaces: RepositoryWorkspaces
    @Environment(\.openWindow) private var openWindow
    @State private var snapshotError: String?
    @State private var showSnapshotError = false
    @State private var openingError: String?
    @State private var didOpenCaptureView = false
    @AppStorage("taskFilesAutosave") private var taskFilesAutosave = true
    @State private var appearance: AppearanceStore
    /// `lf desktop`'s socket, held for the app's lifetime.
    @State private var desktopServer: DesktopServer?

    init() {
        NSWindow.allowsAutomaticWindowTabbing = false
        Perf.begin(Perf.coldStart, "launch", id: "launch",
                   detail: "pre_main_ms=\(Int(Perf.millisecondsSinceProcessStart() ?? -1))")
        Perf.begin(Perf.workspaceCurrent, "launch", id: "launch")
        if AppTestMode.current() == nil {
            DesktopLog.home.begin()
            LaunchJournal.home.begin()
            LaunchJournal.recordExits()
            DesktopVitals.home.start()
        }
        bootstrapLoopflowApp()
        _workspaces = State(initialValue: RepositoryWorkspaces(
            localMachineId: AppTestMode.shouldBypassRegistry ? nil : WorkCache.home.load()?.machineId))
        // Enrich our own process PATH before any children spawn, so tools launched
        // by Wave launchers can find git and agent CLIs that live in
        // Homebrew or ~/.local/bin.
        enrichProcessPathForGUILaunch()
        // A reboot leaves interrupted Sessions behind; bring them all back unasked.
        if AppTestMode.current() == nil {
            Task.detached(priority: .utility) {
                do {
                    try await RegistryQueryLocal.shared.resumeInterrupted()
                } catch {
                    DesktopLog.home.error("Resuming interrupted Sessions failed", ["error": String(describing: error)])
                }
            }
        }
        // Ghostty borrows the process environment when appearance initializes it.
        // Finish environment changes before publishing the initial theme.
        _appearance = State(initialValue: AppearanceStore.shared)
        if AppTestMode.current() == nil {
            _desktopServer = State(initialValue: DesktopServer.start())
        }
    }

    var body: some Scene {
        let launchRepoURL = LaunchArguments.repoURL()
        let registryQuery = RegistryQueryLocal.shared

        WindowGroup(id: "workspace", for: RepositoryWorkspace.self) { $workspace in
            Group {
                // A capture opens its target window from a window that stays:
                // the launch view hands off to a repository window, and opening
                // another window during that hand-off aborts in AttributeGraph.
                if let workspace {
                    RepositoryWorkspaceView(workspace: workspace, portfolioService: portfolioService,
                                            workspaces: workspaces,
                                            query: registryQuery, router: taskLinks,
                                            openRepository: requestRepository)
                        .task { openCaptureViewIfNeeded() }
                } else if AppTestMode.shouldBypassRegistry {
                    RepoView(portfolioService: portfolioService,
                             sessionWorkspaces: workspaces.registry(for: "fixture"), initialRepoPath: launchRepoURL?.path,
                             query: registryQuery)
                        .task { openCaptureViewIfNeeded() }
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
            .modifier(AppAppearance(appearance: appearance))
            .onOpenURL { handleDeepLink($0) }
            .uiTestWindowWidth()
            .uiTestSnapshot()
        }
        .windowStyle(.automatic)
        .windowToolbarStyle(.unifiedCompact(showsTitle: false))
        .defaultSize(width: 1280, height: 800)
        .commands {
            ConsoleCommands()
            CommandGroup(replacing: .appInfo) {
                Button("About Loopflow") { openWindow(id: "about-loopflow") }
            }

            CommandGroup(replacing: .newItem) {
                Button("Open Repo…") { openRepoPanel() }
                    .keyboardShortcut("o", modifiers: [.command, .shift])
            }
            CommandGroup(replacing: .help) {
                Button("Keyboard Shortcuts") { openWindow(id: "keyboard-shortcuts") }
                Button("Snapshot for Review") {
                    snapshotCurrentWindow()
                }
            }

            if RenderFlash.available {
                CommandMenu("Debug") {
                    Toggle("Flash Re-renders", isOn: Binding(
                        get: { RenderFlash.shared.enabled }, set: { RenderFlash.shared.setEnabled($0) }))
                        .keyboardShortcut("r", modifiers: [.command, .option, .control])
                }
            }

            CommandMenu("Go") {
                Button("Portfolio") {
                    openWindow(id: "portfolio")
                }

                Button("Telemetry") {
                    openWindow(id: "telemetry")
                }

                if !portfolioService.repos.isEmpty {
                    Menu("Move to Repo") {
                        ForEach(portfolioService.repos) { repo in
                            Button(repo.displayName) {
                                requestRepository(repo.path, nil)
                            }
                            .disabled(repo.error != nil)
                            .help(repo.error ?? repo.path)
                        }
                    }
                }

            }
        }

        Window("About Loopflow", id: "about-loopflow") {
            AboutLoopflowView()
        }
        .windowResizability(.contentSize)

        Window("Open Source Licenses", id: "open-source-licenses") {
            OpenSourceLicensesView()
        }
        .defaultSize(width: 760, height: 600)

        Window("Portfolio", id: "portfolio") {
            WavesView(portfolioService: portfolioService, openTask: { path, id in
                requestRepository(path, .task(TaskLink(issue: id, repo: path)))
            })
                .modifier(AppAppearance(appearance: appearance))
                .onOpenURL { handleDeepLink($0) }
        }
        .defaultSize(width: 1080, height: 760)

        Window("Open Work", id: "open-work") {
            WorkspaceOpeningFeedback(error: $openingError)
                .modifier(AppAppearance(appearance: appearance))
        }
        .defaultSize(width: 480, height: 240)

        Settings {
            TabView {
                ThemeChooser(appearance: appearance).tabItem { Label("Appearance", systemImage: "paintpalette") }
                Form { Toggle("Autosave Task Files", isOn: $taskFilesAutosave) }
                    .padding(24).tabItem { Label("Editing", systemImage: "pencil") }
                KeyboardShortcutsView().tabItem { Label("Keyboard Shortcuts", systemImage: "keyboard") }
            }.modifier(AppAppearance(appearance: appearance))
        }
        Window("Keyboard Shortcuts", id: "keyboard-shortcuts") { KeyboardShortcutsView() }
            .defaultSize(width: 540, height: 480)

        Window("Telemetry", id: "telemetry") {
            TelemetryDashboardView()
                .modifier(AppAppearance(appearance: appearance))
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

    /// Work addresses resolve only through Rust's shared owners (`lf open --link`).
    /// An ambiguous or unknown address changes nothing and reports the qualified choices.
    @MainActor
    private func handleDeepLink(_ url: URL) {
        guard url.scheme == "loopflow" else { return }
        switch url.host {
        case "open" where url.path.isEmpty && viewItems(url) != nil:
            // `lf desktop view` names a selection expression, not a Work address.
            guard let (repoPath, expression) = viewItems(url) else { return }
            requestRepository(repoPath, .view(expression: expression, address: url.absoluteString))
        case "portfolio" where url.path.isEmpty:
            openWindow(id: "portfolio")
        case "sessions" where url.path.isEmpty:
            NotificationCenter.default.post(name: .openSessions, object: nil)
        default:
            taskLinks.receiveAddress { isLatest in
                do {
                    let query = RegistryQueryLocal.shared
                    let resolved = try await query.workLink(url)
                    let link = LinkedWork(resolved)
                    var path = resolved.target.repositoryPath
                    if case .task(let task) = link, let repo = task.repo { path = repo }
                    try await openRepository(path, link: link, address: resolved.url, raises: isLatest)
                } catch { reportOpeningError(error) }
            }
        }
    }

    private func viewItems(_ url: URL) -> (repo: String, expression: String)? {
        let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        guard let repo = items.first(where: { $0.name == "repo" })?.value,
              let expression = items.first(where: { $0.name == "view" })?.value else { return nil }
        return (repo, expression)
    }

    @MainActor
    private func reportOpeningError(_ error: Error) {
        guard !(error is CancellationError) else { return }
        openingError = error.localizedDescription
        openWindow(id: "open-work")
    }

    @MainActor
    private func requestRepository(_ path: String, _ link: LinkedWork?) {
        Task {
            do { try await openRepository(path, link: link) }
            catch { reportOpeningError(error) }
        }
    }

    @MainActor
    private func openRepository(_ path: String, link: LinkedWork? = nil, address: String? = nil,
                                raises: @MainActor () -> Bool = { true }) async throws {
        let query = RegistryQueryLocal.shared
        let workspace = try await taskLinks.openRepository(path: path, link: link, address: address, query: query,
                                                           raises: raises) { workspace in
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
        requestRepository(url.path, nil)
    }
}

/// Resolve every window from the app-owned theme, including native controls.
struct AppAppearance: ViewModifier {
    let appearance: AppearanceStore
    @Environment(\.openSettings) private var openSettings

    func body(content: Content) -> some View {
        let theme = appearance.effectiveTheme
        content
            .preferredColorScheme(theme.colorScheme)
            .environment(\.colorScheme, theme.colorScheme)
            .environment(\.palette, theme.palette)
            .environment(\.theme, theme)
            .tint(theme.palette.accentInk)
            .foregroundStyle(theme.palette.text)
            .background(theme.palette.background)
            .safeAreaInset(edge: .top) {
                if let notice = appearance.notice, !appearance.chooserOpen {
                    HStack {
                        Text(notice).font(.caption)
                        Button("Choose Theme…") {
                            appearance.beginPreview()
                            openSettings()
                        }
                    }
                    .padding(8).foregroundStyle(theme.palette.text)
                    .background(theme.palette.surface)
                }
            }
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
                // A launch view that hands off to a repository window is
                // replaced before it settles; the replacement captures.
                if Task.isCancelled { return }

                if let window = captureWindow {
                    if let width = AppTestMode.windowWidth {
                        let height = AppTestMode.windowHeight
                            ?? window.contentView?.frame.height
                            ?? window.frame.height
                        window.setContentSize(NSSize(width: width, height: height))
                    }
                    try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
                    if Task.isCancelled { return }
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
