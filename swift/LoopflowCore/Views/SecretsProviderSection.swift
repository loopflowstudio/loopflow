import SwiftUI

public struct SecretsProviderSection: View {
    @Environment(\.palette) private var palette
    @Bindable var store: SecretsProviderStore

    @State private var token = ""
    @State private var project = ""
    @State private var config = ""

    public init(store: SecretsProviderStore) {
        self.store = store
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            Text("Secrets Provider")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            if store.status.connected {
                connectedView
            } else {
                connectForm
            }

            if let error = store.error {
                Text(error)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
            }
        }
    }

    private var connectedView: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Circle()
                    .fill(Color.statusSuccess)
                    .frame(width: 8, height: 8)
                Text("Doppler")
                    .font(Typography.sectionTitle(18))
                    .foregroundStyle(palette.text)
                Spacer(minLength: 0)
                Button("Disconnect") {
                    Task { await store.disconnect() }
                }
                .buttonStyle(DestructiveButtonStyle())
                .frame(minHeight: HitTarget.comfortable)
            }

            if let project = store.status.project, let config = store.status.config {
                Text("\(project) / \(config)")
                    .font(Typography.code(13))
                    .foregroundStyle(palette.textSecondary)
            }

            ForEach(store.status.keys) { key in
                HStack(spacing: Spacing.sm) {
                    Circle()
                        .fill(key.present ? Color.statusSuccess : Color.statusNeutral)
                        .frame(width: 8, height: 8)
                    Text(key.envName)
                        .font(Typography.code(12))
                        .foregroundStyle(palette.text)
                    Spacer(minLength: 0)
                    Text(key.present ? "Present" : "Missing")
                        .font(Typography.caption())
                        .foregroundStyle(key.present ? palette.textSecondary : Color.statusWarning)
                }
            }

            HStack(spacing: Spacing.sm) {
                Button("Refresh") {
                    Task { await store.sync() }
                }
                .buttonStyle(GhostButtonStyle())
                .disabled(store.isSyncing)

                if store.isSyncing {
                    ProgressView()
                        .controlSize(.small)
                }
            }
        }
        .padding(Spacing.md)
        .background(palette.surface)
        .overlay(
            RoundedRectangle(cornerRadius: CornerRadius.lg)
                .stroke(palette.border, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
    }

    private var connectForm: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(spacing: Spacing.sm) {
                Circle()
                    .fill(Color.statusNeutral)
                    .frame(width: 8, height: 8)
                Text("Doppler")
                    .font(Typography.sectionTitle(18))
                    .foregroundStyle(palette.text)
            }

            Text("Connect a Doppler project to auto-populate Claude and Codex API keys.")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            SecureField("Service token", text: $token)
                .textFieldStyle(.plain)
                .font(Typography.code(13))
                .padding(Spacing.md)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))

            TextField("Project", text: $project)
                .textFieldStyle(.plain)
                .font(Typography.code(13))
                .padding(Spacing.md)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))

            TextField("Config", text: $config)
                .textFieldStyle(.plain)
                .font(Typography.code(13))
                .padding(Spacing.md)
                .background(palette.surfaceMuted)
                .clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))

            Button("Connect") {
                Task {
                    await store.connect(
                        provider: "doppler",
                        token: token,
                        project: project,
                        config: config
                    )
                    if store.status.connected {
                        token = ""
                    }
                }
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(token.isEmpty || project.isEmpty || config.isEmpty || store.isSyncing)
        }
        .padding(Spacing.md)
        .background(palette.surface)
        .overlay(
            RoundedRectangle(cornerRadius: CornerRadius.lg)
                .stroke(palette.border, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
    }
}
