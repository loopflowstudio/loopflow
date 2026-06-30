#if os(iOS)
import SwiftUI
import LoopflowCore

struct DiscoveryView: View {
    @Environment(RepoState.self) private var repoState
    @Environment(OutputBuffer.self) private var outputBuffer

    @State private var serverURL = "https://lfd.example.com"
    @State private var token = ""
    @State private var isConnecting = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("https://lfd.example.com", text: $serverURL)
                        .textContentType(.URL)
                        .keyboardType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()

                    SecureField("Bearer token", text: $token)
                        .textContentType(.password)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                } header: {
                    Text("Self-hosted lfd")
                } footer: {
                    Text("Start lfd in the repo deployment and paste the URL and LFD_AUTH_TOKEN from Doppler or your local secret store.")
                }

                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .foregroundStyle(Color.statusError)
                    }
                }

                Section {
                    Button {
                        connect()
                    } label: {
                        if isConnecting {
                            ProgressView()
                        } else {
                            Text("Connect")
                        }
                    }
                    .disabled(isConnecting || token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
            .navigationTitle("Connection")
        }
        .tint(.loopflowBurgundy)
    }

    private func connect() {
        guard let connection = makeConnection() else {
            errorMessage = "Enter a valid http or https lfd URL."
            return
        }

        isConnecting = true
        errorMessage = nil

        Task {
            do {
                try await repoState.connect(to: connection, outputBuffer: outputBuffer)
                await MainActor.run {
                    isConnecting = false
                }
            } catch {
                await MainActor.run {
                    isConnecting = false
                    errorMessage = error.localizedDescription
                }
            }
        }
    }

    private func makeConnection() -> ServerConnection? {
        let raw = serverURL.trimmingCharacters(in: .whitespacesAndNewlines)
        guard var components = URLComponents(string: raw),
              let scheme = components.scheme?.lowercased(),
              scheme == "http" || scheme == "https",
              let host = components.host
        else {
            return nil
        }

        let useTLS = scheme == "https"
        let port = components.port ?? (useTLS ? 443 : 80)
        components.path = ""
        components.query = nil
        components.fragment = nil

        return ServerConnection(
            host: host,
            port: port,
            useTLS: useTLS,
            authMode: .staticToken,
            staticToken: token.trimmingCharacters(in: .whitespacesAndNewlines)
        )
    }
}
#endif
