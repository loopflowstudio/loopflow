#if os(iOS)
import SwiftUI
import UIKit
import LoopflowCore

struct MobileRootView: View {
    @State private var repoState = RepoState()
    @State private var outputBuffer = OutputBuffer()
    @State private var selectedWaveId: String?
    @State private var selectedTab = 0
    @State private var showingSettings = false

    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.colorScheme) private var systemScheme
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @AppStorage("appearanceMode") private var appearanceMode = AppearanceMode.system.rawValue

    private var theme: (preferredScheme: ColorScheme?, palette: LoopflowPalette) {
        AppearanceMode.resolvedTheme(rawValue: appearanceMode, systemScheme: systemScheme)
    }

    private var isPadLayout: Bool {
        UIDevice.current.userInterfaceIdiom == .pad || horizontalSizeClass == .regular
    }

    /// True only on first launch before the user has ever connected.
    private var needsInitialSetup: Bool {
        repoState.repoTarget == nil
    }

    var body: some View {
        Group {
            if needsInitialSetup {
                DiscoveryView()
            } else if isPadLayout {
                iPadLayout
            } else {
                iPhoneLayout
            }
        }
        .overlay(alignment: .top) {
            if !needsInitialSetup {
                ConnectionBanner(repoState: repoState, outputBuffer: outputBuffer)
            }
        }
        .environment(repoState)
        .environment(outputBuffer)
        .onChange(of: scenePhase) { _, phase in
            guard phase == .active, !needsInitialSetup else { return }
            Task {
                await repoState.checkConnectionHealth()
            }
        }
        .preferredColorScheme(theme.preferredScheme)
        .environment(\.palette, theme.palette)
    }

    private var iPhoneLayout: some View {
        TabView(selection: $selectedTab) {
            NavigationStack {
                MobileWaveListView(selectedWaveId: $selectedWaveId)
                    .navigationDestination(for: String.self) { waveId in
                        MobileWaveDetailView(waveId: waveId)
                    }
            }
            .tabItem {
                Label("Waves", systemImage: "waveform.path")
            }
            .tag(0)

            ConnectionSetupView()
                .tabItem {
                    Label("Settings", systemImage: "gearshape")
                }
                .tag(1)
        }
        .onChange(of: selectedWaveId) { _, newValue in
            guard newValue != nil else { return }
            selectedTab = 0
        }
    }

    private var iPadLayout: some View {
        NavigationSplitView {
            MobileWaveListView(selectedWaveId: $selectedWaveId)
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button {
                            showingSettings = true
                        } label: {
                            Image(systemName: "gearshape")
                        }
                    }
                }
        } detail: {
            if let selectedWaveId {
                MobileWaveDetailView(waveId: selectedWaveId)
            } else {
                ContentUnavailableView("Select a Wave", systemImage: "waveform.path.ecg")
            }
        }
        .sheet(isPresented: $showingSettings) {
            ConnectionSetupView()
        }
    }
}

// MARK: - Connection Banner

private struct ConnectionBanner: View {
    let repoState: RepoState
    let outputBuffer: OutputBuffer

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isReconnecting = false

    private var bannerIcon: String? {
        switch repoState.connectionState {
        case .connected, .connecting:
            return nil
        case .reconnecting:
            return "arrow.trianglehead.2.counterclockwise"
        case .authFailed:
            return "lock.slash"
        case .disconnected, .trustRequired:
            return "wifi.slash"
        }
    }

    var body: some View {
        if let bannerIcon {
            HStack(spacing: Spacing.sm) {
                Image(systemName: bannerIcon)
                    .foregroundStyle(.white)

                Text(repoState.connectionSummary)
                    .font(Typography.caption())
                    .foregroundStyle(.white)
                    .lineLimit(1)

                Spacer()

                Button {
                    reconnect()
                } label: {
                    if isReconnecting {
                        ProgressView()
                            .tint(.white)
                            .controlSize(.small)
                    } else {
                        Text("Reconnect")
                            .font(Typography.caption())
                            .foregroundStyle(.white)
                    }
                }
                .disabled(isReconnecting)
            }
            .padding(.horizontal, Spacing.lg)
            .padding(.vertical, Spacing.sm)
            .background(Color.statusError.opacity(0.9))
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
            .padding(.horizontal, Spacing.lg)
            .padding(.top, Spacing.xs)
            .transition(.move(edge: .top).combined(with: .opacity))
            .animation(reduceMotion ? nil : .easeInOut(duration: 0.2), value: bannerIcon != nil)
        }
    }

    private func reconnect() {
        isReconnecting = true
        Task {
            defer { isReconnecting = false }
            try? await repoState.connectLfd(outputBuffer: outputBuffer)
        }
    }
}
#endif
