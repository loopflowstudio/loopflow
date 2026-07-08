import SwiftUI

public struct ProviderRow: View {
    @Environment(\.palette) private var palette

    public let status: AuthProviderStatus
    public let pendingFlow: AuthFlow?
    public let isEnabled: Bool
    public let error: String?
    public let showURLFallback: Bool
    public let onConnect: (AuthProvider) -> Void
    public let onDisconnect: (AuthProvider) -> Void
    public let onCancel: (AuthProvider) -> Void
    public let onToggle: ((AuthProvider, Bool) -> Void)?
    public let onCopy: (String) -> Void

    public init(
        status: AuthProviderStatus,
        pendingFlow: AuthFlow? = nil,
        isEnabled: Bool = true,
        error: String? = nil,
        showURLFallback: Bool = false,
        onConnect: @escaping (AuthProvider) -> Void,
        onDisconnect: @escaping (AuthProvider) -> Void,
        onCancel: @escaping (AuthProvider) -> Void,
        onToggle: ((AuthProvider, Bool) -> Void)? = nil,
        onCopy: @escaping (String) -> Void
    ) {
        self.status = status
        self.pendingFlow = pendingFlow
        self.isEnabled = isEnabled
        self.error = error
        self.showURLFallback = showURLFallback
        self.onConnect = onConnect
        self.onDisconnect = onDisconnect
        self.onCancel = onCancel
        self.onToggle = onToggle
        self.onCopy = onCopy
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            HStack(spacing: Spacing.sm) {
                Image(systemName: status.provider.icon)
                    .foregroundStyle(palette.text)
                    .frame(width: 20)

                Text(status.provider.displayName)
                    .font(Typography.body())
                    .foregroundStyle(palette.text)

                Spacer(minLength: 0)

                statusBadge

                action
            }
            .frame(minHeight: HitTarget.comfortable)

            if status.status == .pending, let flow = pendingFlow {
                pendingDetail(flow)
            }

            if let error {
                Text(error)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
                    .padding(.leading, 28)
            }
        }
        .opacity(isEnabled ? 1 : 0.6)
    }

    private var statusBadge: some View {
        HStack(spacing: Spacing.xs) {
            Circle()
                .fill(statusColor)
                .frame(width: 6, height: 6)
            Text(statusLabel)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
        }
    }

    @ViewBuilder
    private var action: some View {
        switch status.status {
        case .active:
            if let onToggle {
                Toggle("", isOn: Binding(
                    get: { isEnabled },
                    set: { onToggle(status.provider, $0) }
                ))
                .toggleStyle(.switch)
                .labelsHidden()
                .controlSize(.small)
            } else {
                Button("Disconnect") {
                    onDisconnect(status.provider)
                }
                .buttonStyle(GhostButtonStyle())
                .controlSize(.small)
            }

        case .pending:
            Button("Cancel") {
                onCancel(status.provider)
            }
            .buttonStyle(GhostButtonStyle())
            .controlSize(.small)

        case .none, .expired:
            Button(status.status == .expired ? "Reconnect" : "Connect") {
                onConnect(status.provider)
            }
            .buttonStyle(DarkButtonStyle())
            .controlSize(.small)
        }
    }

    private func pendingDetail(_ flow: AuthFlow) -> some View {
        VStack(alignment: .leading, spacing: Spacing.xs) {
            if let userCode = flow.userCode {
                HStack(spacing: Spacing.sm) {
                    Text(userCode)
                        .font(Typography.code())
                        .foregroundStyle(palette.text)
                    Button("Copy") { onCopy(userCode) }
                        .buttonStyle(GhostButtonStyle())
                        .controlSize(.small)
                }
            }

            Text("Complete authentication in your browser.")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            if showURLFallback {
                let url = flow.verificationURIComplete ?? flow.verificationURI
                Text(url)
                    .font(Typography.code(11))
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)
                    .lineLimit(2)
            }
        }
        .padding(.leading, 28)
    }

    private var statusColor: Color {
        switch status.status {
        case .active:
            status.credentialType == .apikey ? Color.statusWarning : Color.statusSuccess
        case .pending:
            Color.statusWarning
        case .none, .expired:
            Color.statusNeutral
        }
    }

    private var statusLabel: String {
        switch status.status {
        case .active:
            if let login = status.login { login }
            else if status.credentialType == .apikey { "API Key" }
            else { "Connected" }
        case .pending: "Pending"
        case .none: "Not connected"
        case .expired: "Expired"
        }
    }
}
