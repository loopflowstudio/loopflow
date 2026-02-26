import SwiftUI

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

public struct AuthProviderCard: View {
    @Environment(\.palette) private var palette

    public let status: AuthProviderStatus
    public let pendingFlow: AuthFlow?
    public let isEnabled: Bool
    public let error: String?
    public let showURLFallback: Bool
    public let onConnect: (AuthProvider) -> Void
    public let onDisconnect: (AuthProvider) -> Void
    public let onCancel: (AuthProvider) -> Void

    public init(
        status: AuthProviderStatus,
        pendingFlow: AuthFlow?,
        isEnabled: Bool,
        error: String? = nil,
        showURLFallback: Bool = false,
        onConnect: @escaping (AuthProvider) -> Void,
        onDisconnect: @escaping (AuthProvider) -> Void,
        onCancel: @escaping (AuthProvider) -> Void
    ) {
        self.status = status
        self.pendingFlow = pendingFlow
        self.isEnabled = isEnabled
        self.error = error
        self.showURLFallback = showURLFallback
        self.onConnect = onConnect
        self.onDisconnect = onDisconnect
        self.onCancel = onCancel
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: Spacing.sm) {
            HStack(alignment: .top, spacing: Spacing.sm) {
                Image(systemName: status.provider.icon)
                    .foregroundStyle(palette.text)
                    .frame(minWidth: HitTarget.minimum, minHeight: HitTarget.minimum)

                VStack(alignment: .leading, spacing: Spacing.xs) {
                    Text(status.provider.displayName)
                        .font(Typography.sectionTitle(18))
                        .foregroundStyle(palette.text)

                    HStack(spacing: Spacing.xs) {
                        Circle()
                            .fill(statusColor)
                            .frame(width: 8, height: 8)
                        Text(statusLabel)
                            .font(Typography.caption())
                            .foregroundStyle(palette.textSecondary)
                    }
                }

                Spacer(minLength: 0)
                actionButton
            }

            stateBody

            if let error {
                Text(error)
                    .font(Typography.caption())
                    .foregroundStyle(Color.statusError)
            }
        }
        .padding(Spacing.md)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.surface)
        .overlay(
            RoundedRectangle(cornerRadius: CornerRadius.lg)
                .stroke(palette.border, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        .opacity(isEnabled ? 1 : 0.7)
    }

    @ViewBuilder
    private var stateBody: some View {
        switch status.status {
        case .active:
            Text(status.login ?? "Connected")
                .font(Typography.body())
                .foregroundStyle(palette.textSecondary)

        case .pending:
            pendingBody

        case .none, .expired:
            Text(status.status == .expired ? "Connection expired" : "Not connected")
                .font(Typography.body())
                .foregroundStyle(palette.textSecondary)
        }
    }

    @ViewBuilder
    private var pendingBody: some View {
        if let flow = pendingFlow {
            VStack(alignment: .leading, spacing: Spacing.sm) {
                if let userCode = flow.userCode {
                    HStack(spacing: Spacing.sm) {
                        Text(userCode)
                            .font(Typography.code())
                            .foregroundStyle(palette.text)

                        copyButton(
                            "Copy",
                            value: userCode,
                            accessibilityLabel: "Copy code",
                            accessibilityHint: "Copies the verification code"
                        )
                    }
                }

                Text("Complete authentication in your browser.")
                    .font(Typography.body())
                    .foregroundStyle(palette.textSecondary)

                if showURLFallback {
                    fallbackURLView(flow)
                }
            }
        } else {
            Text("Auth pending (possibly started from another client).")
                .font(Typography.body())
                .foregroundStyle(palette.textSecondary)
        }
    }

    @ViewBuilder
    private func fallbackURLView(_ flow: AuthFlow) -> some View {
        let flowURL = flow.verificationURIComplete ?? flow.verificationURI

        VStack(alignment: .leading, spacing: Spacing.xs) {
            Text("Browser did not open. Copy this URL:")
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)

            HStack(spacing: Spacing.sm) {
                Text(flowURL)
                    .font(Typography.code(11))
                    .foregroundStyle(palette.text)
                    .textSelection(.enabled)
                    .lineLimit(2)

                copyButton(
                    "Copy",
                    value: flowURL,
                    accessibilityLabel: "Copy URL",
                    accessibilityHint: "Copies the provider verification URL"
                )
            }
        }
    }

    @ViewBuilder
    private var actionButton: some View {
        switch status.status {
        case .active:
            Button("Disconnect") {
                onDisconnect(status.provider)
            }
            .buttonStyle(DestructiveButtonStyle())
            .disabled(!isEnabled)
            .frame(minHeight: HitTarget.comfortable)

        case .pending:
            Button("Cancel") {
                onCancel(status.provider)
            }
            .buttonStyle(GhostButtonStyle())
            .disabled(!isEnabled)
            .frame(minHeight: HitTarget.comfortable)

        case .none, .expired:
            Button(status.status == .expired ? "Reconnect" : "Connect") {
                onConnect(status.provider)
            }
            .buttonStyle(DarkButtonStyle())
            .disabled(!isEnabled)
            .frame(minHeight: HitTarget.comfortable)
        }
    }

    private var statusColor: Color {
        switch status.status {
        case .active:
            Color.statusSuccess
        case .pending:
            Color.statusWarning
        case .none, .expired:
            Color.statusNeutral
        }
    }

    private var statusLabel: String {
        switch status.status {
        case .active:
            "Connected"
        case .pending:
            "Pending"
        case .none:
            "Not connected"
        case .expired:
            "Expired"
        }
    }

    private func copyButton(
        _ title: LocalizedStringKey,
        value: String,
        accessibilityLabel: String,
        accessibilityHint: String
    ) -> some View {
        Button(title) {
            copyToClipboard(value)
        }
        .buttonStyle(GhostButtonStyle())
        .accessibilityLabel(accessibilityLabel)
        .accessibilityHint(accessibilityHint)
        .minHitTarget()
    }

    private func copyToClipboard(_ value: String) {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(value, forType: .string)
        #elseif os(iOS)
        UIPasteboard.general.string = value
        #endif
    }
}
