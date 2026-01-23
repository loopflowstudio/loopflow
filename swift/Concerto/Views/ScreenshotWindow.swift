// Window for screenshot capture mode.
// Loads a repo, waits for UI to stabilize, captures, and exits.

import SwiftUI
import LoopflowCore
import os.log

private let logger = Logger(subsystem: "com.loopflow.concerto", category: "screenshot")

struct ScreenshotWindow: View {
    let mode: AppState.ScreenshotMode
    let recentsService: RecentsService
    @State private var appState = AppState()
    @State private var hasLoaded = false
    @State private var hasCaptured = false

    var body: some View {
        ContentView(appState: appState)
            .background(WindowAccessor(onWindowReady: { window in
                Task { @MainActor in
                    await setupAndCapture(window: window)
                }
            }))
    }

    private func setupAndCapture(window: NSWindow) async {
        guard !hasLoaded else { return }
        hasLoaded = true

        // Open the repo if specified
        if let repoPath = mode.repoPath {
            let repoURL = URL(fileURLWithPath: (repoPath as NSString).expandingTildeInPath)
            await appState.openRepo(repoURL)
        }

        // Configure mock agents if requested
        if mode.mockLoops {
            appState.configureMockAgents()
        }

        // Select a specific worktree if requested
        if let branch = mode.selectBranch {
            appState.selectedWorktree = appState.worktrees.first { $0.branch == branch }
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

        // Wait for UI to stabilize
        try? await Task.sleep(for: .seconds(1))

        // Capture and exit
        await captureAndExit(window: window)
    }

    @MainActor
    private func captureAndExit(window: NSWindow) async {
        guard !hasCaptured else { return }
        hasCaptured = true

        let captureService = CaptureService()
        do {
            _ = try captureService.captureWindow(window, to: mode.outputPath)
            logger.info("screenshot saved to: \(mode.outputPath)")
        } catch {
            logger.error("capture failed: \(error.localizedDescription)")
        }

        // Exit the app
        NSApp.terminate(nil)
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
