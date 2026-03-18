#if os(iOS)
import SwiftUI
import LoopflowCore

struct ConnectionSetupView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    var body: some View {
        ScrollView {
            ConnectionsPanel(
                authStore: repoState.authProviderStore,
                secretsStore: repoState.secretsProviderStore,
                onConnect: { Task { await repoState.authProviderStore.connect($0) } },
                onDisconnect: { Task { await repoState.authProviderStore.disconnect($0) } },
                onCancel: { Task { await repoState.authProviderStore.disconnect($0) } },
                onCopy: { copyToClipboard($0) }
            )
            .padding(Spacing.lg)
        }
        .navigationTitle("Connections")
        .task {
            await repoState.secretsProviderStore.refresh()
        }
    }
}
#endif
