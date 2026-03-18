#if os(iOS)
import SwiftUI
import LoopflowCore

struct ConnectionSetupView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(\.palette) private var palette

    var body: some View {
        List {
            Section("Provider Auth") {
                ForEach(repoState.authProviderStore.providers) { provider in
                    AuthProviderCard(provider: provider)
                }
            }

            Section("Secrets") {
                SecretsProviderSection(store: repoState.secretsProviderStore)
                    .listRowInsets(EdgeInsets())
                    .listRowBackground(Color.clear)
            }
        }
        .navigationTitle("Connections")
        .task {
            await repoState.secretsProviderStore.refresh()
        }
    }
}
#endif
