// Window for screenshot snapshot mode.
// Loads a repo, waits for UI to stabilize, snapshots, and exits.

import SwiftUI
import LoopflowCore
import os.log

private let logger = Logger(subsystem: "com.loopflow.concerto", category: "screenshot")

// Environment key to override initial tab in WaveDetailPanel for screenshots
struct ScreenshotTabKey: EnvironmentKey {
    static let defaultValue: String? = nil
}

extension EnvironmentValues {
    var screenshotTab: String? {
        get { self[ScreenshotTabKey.self] }
        set { self[ScreenshotTabKey.self] = newValue }
    }
}

struct ScreenshotWindow: View {
    let mode: RepoState.ScreenshotMode
    let recentsService: RecentsService
    @State private var repoState = RepoState()
    @State private var outputBuffer = OutputBuffer()
    @State private var hasLoaded = false
    @State private var hasCaptured = false

    var body: some View {
        // Use HStack layout for screenshot mode - NavigationSplitView doesn't capture properly
        ScreenshotLayout()
            .environment(repoState)
            .environment(outputBuffer)
            .environment(\.screenshotTab, mode.selectTab)
            .task {
                await setupState()
            }
            .background(WindowAccessor(onWindowReady: { window in
                Task { @MainActor in
                    await captureWhenReady(window: window)
                }
            }))
    }

    /// Configure state before the view fully renders.
    private func setupState() async {
        guard !hasLoaded else { return }
        hasLoaded = true

        // Open the repo if specified (skip background refresh in mock mode to avoid overwriting mock data)
        if let repoPath = mode.repoPath {
            let repoURL = URL(fileURLWithPath: (repoPath as NSString).expandingTildeInPath)
            await repoState.openRepo(repoURL, outputBuffer: outputBuffer, skipBackgroundRefresh: mode.mockLoops)
        }

        // Configure mock waves if requested
        if mode.mockLoops {
            if mode.mockConfig == "empty" {
                repoState.configureMockWavesEmpty()
            } else {
                repoState.configureMockWaves()
            }
        }

        // Select a specific wave if requested
        if let branch = mode.selectBranch {
            repoState.selectedWave = repoState.waves.first { $0.branch == branch }
        }
    }

    /// Wait for view to stabilize and capture.
    private func captureWhenReady(window: NSWindow) async {
        // Wait for state setup to complete
        while !hasLoaded {
            try? await Task.sleep(for: .milliseconds(50))
        }

        // Resize window if specified
        if let (width, height) = mode.windowSize {
            let newFrame = NSRect(
                x: window.frame.origin.x,
                y: window.frame.origin.y,
                width: CGFloat(width),
                height: CGFloat(height)
            )
            window.setFrame(newFrame, display: true, animate: false)
        }

        // Make window visible and key to trigger SwiftUI rendering
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        // Wait for SwiftUI to render the view with updated state
        try? await Task.sleep(for: .seconds(2))

        // Capture and exit
        await snapshotAndExit(window: window)
    }

    @MainActor
    private func snapshotAndExit(window: NSWindow) async {
        guard !hasCaptured else { return }
        hasCaptured = true

        let snapshotService = SnapshotService()
        do {
            _ = try snapshotService.snapshotWindow(window, to: mode.outputPath)
            logger.info("snapshot saved to: \(mode.outputPath)")
        } catch {
            logger.error("snapshot failed: \(error.localizedDescription)")
        }

        // Exit the app
        NSApp.terminate(nil)
    }
}

/// Layout for screenshot capture that uses HStack instead of NavigationSplitView.
/// NavigationSplitView's sidebar doesn't capture properly with bitmapImageRepForCachingDisplay.
private struct ScreenshotLayout: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    var body: some View {
        HStack(spacing: 0) {
            // Sidebar
            WaveSidebar()
                .frame(width: 280)

            // Divider
            Rectangle()
                .fill(palette.border)
                .frame(width: 1)

            // Detail
            detailContent
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(palette.background)
    }

    @ViewBuilder
    private var detailContent: some View {
        if repoState.isLoading {
            ProgressView("Loading...")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if let wave = repoState.selectedWave {
            WaveDetailPanel(wave: wave)
                .id(wave.id)
        } else {
            StartWaveView()
        }
    }
}

/// Helper to get the hosting window when it becomes available.
private struct WindowAccessor: NSViewRepresentable {
    let onWindowReady: (NSWindow) -> Void

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            if let window = view.window {
                onWindowReady(window)
            }
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        if let window = nsView.window {
            onWindowReady(window)
        }
    }
}
