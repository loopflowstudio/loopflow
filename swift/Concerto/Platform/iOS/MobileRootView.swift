#if os(iOS)
import SwiftUI
import UIKit
import LoopflowCore

struct MobileRootView: View {
    @State private var repoState = RepoState()
    @State private var outputBuffer = OutputBuffer()
    @State private var profileStore = MobileConnectionProfilesStore()
    @State private var selectedWaveId: String?
    @State private var selectedTab = 0
    @State private var showingSettings = false

    @Environment(\.colorScheme) private var systemScheme
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @AppStorage("appearanceMode") private var appearanceMode = AppearanceMode.system.rawValue

    private var theme: (preferredScheme: ColorScheme?, palette: LoopflowPalette) {
        AppearanceMode.resolvedTheme(rawValue: appearanceMode, systemScheme: systemScheme)
    }

    private var isPadLayout: Bool {
        UIDevice.current.userInterfaceIdiom == .pad || horizontalSizeClass == .regular
    }

    private var needsConnectionSetup: Bool {
        !repoState.isConnected || repoState.repoTarget == nil
    }

    var body: some View {
        Group {
            if needsConnectionSetup {
                ConnectionSetupView(profileStore: profileStore)
            } else if isPadLayout {
                iPadLayout
            } else {
                iPhoneLayout
            }
        }
        .environment(repoState)
        .environment(outputBuffer)
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

            ConnectionSetupView(profileStore: profileStore)
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
            ConnectionSetupView(profileStore: profileStore)
        }
    }
}
#endif
