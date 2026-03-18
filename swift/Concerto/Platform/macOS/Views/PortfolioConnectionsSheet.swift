import SwiftUI
import LoopflowCore
import AppKit

struct PortfolioConnectionsSheet: View {
    @Environment(\.palette) private var palette

    let authStore: AuthProviderStore
    let onDismiss: () -> Void

    @State private var browserFallbackProviders: Set<AuthProvider> = []

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Connections")
                    .font(Typography.sectionTitle())
                    .foregroundStyle(palette.text)
                Spacer()
                Button("Done") { onDismiss() }
                    .buttonStyle(GhostButtonStyle())
            }
            .padding(.horizontal, Spacing.xl)
            .padding(.top, Spacing.xl)
            .padding(.bottom, Spacing.lg)

            ScrollView {
                ConnectionsPanel(
                    authStore: authStore,
                    secretsStore: SecretsProviderStore(),
                    browserFallback: browserFallbackProviders,
                    onConnect: connectProvider,
                    onDisconnect: disconnectProvider,
                    onCancel: disconnectProvider,
                    onCopy: copyToClipboard
                )
                .padding(.horizontal, Spacing.xl)
                .padding(.bottom, Spacing.xl)
            }
        }
        .background(palette.background)
        .frame(width: 420)
        .frame(minHeight: 400, idealHeight: 600)
        .onChange(of: authStore.browserLaunchRequest) { _, _ in
            handleBrowserLaunchRequest()
        }
    }

    private func connectProvider(_ provider: AuthProvider) {
        Task { await authStore.connect(provider) }
    }

    private func disconnectProvider(_ provider: AuthProvider) {
        Task { await authStore.disconnect(provider) }
    }

    private func handleBrowserLaunchRequest() {
        guard let launchRequest = authStore.consumeBrowserLaunchRequest() else {
            return
        }
        let opened = NSWorkspace.shared.open(launchRequest.url)
        if opened {
            browserFallbackProviders.remove(launchRequest.provider)
        } else {
            browserFallbackProviders.insert(launchRequest.provider)
        }
    }
}
