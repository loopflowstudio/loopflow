import SwiftUI

public struct ProviderGroupSection<Footer: View>: View {
    @Environment(\.palette) private var palette

    public let role: ProviderRole
    public let providers: [AuthProviderStatus]
    public let pendingFlows: [AuthProvider: AuthFlow]
    public let enabledProviders: Set<AuthProvider>?
    public let errors: (AuthProvider) -> String?
    public let browserFallback: Set<AuthProvider>
    public let onConnect: (AuthProvider) -> Void
    public let onDisconnect: (AuthProvider) -> Void
    public let onCancel: (AuthProvider) -> Void
    public let onToggle: ((AuthProvider, Bool) -> Void)?
    public let onCopy: (String) -> Void
    public let footer: () -> Footer

    public init(
        role: ProviderRole,
        providers: [AuthProviderStatus],
        pendingFlows: [AuthProvider: AuthFlow] = [:],
        enabledProviders: Set<AuthProvider>? = nil,
        errors: @escaping (AuthProvider) -> String? = { _ in nil },
        browserFallback: Set<AuthProvider> = [],
        onConnect: @escaping (AuthProvider) -> Void,
        onDisconnect: @escaping (AuthProvider) -> Void,
        onCancel: @escaping (AuthProvider) -> Void,
        onToggle: ((AuthProvider, Bool) -> Void)? = nil,
        onCopy: @escaping (String) -> Void,
        @ViewBuilder footer: @escaping () -> Footer = { EmptyView() }
    ) {
        self.role = role
        self.providers = providers
        self.pendingFlows = pendingFlows
        self.enabledProviders = enabledProviders
        self.errors = errors
        self.browserFallback = browserFallback
        self.onConnect = onConnect
        self.onDisconnect = onDisconnect
        self.onCancel = onCancel
        self.onToggle = onToggle
        self.onCopy = onCopy
        self.footer = footer
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(role.displayName)
                .font(Typography.caption())
                .foregroundStyle(palette.textSecondary)
                .padding(.bottom, Spacing.sm)

            VStack(spacing: 0) {
                ForEach(Array(providers.enumerated()), id: \.element.id) { index, status in
                    if index > 0 {
                        Divider()
                            .foregroundStyle(palette.border)
                    }
                    ProviderRow(
                        status: status,
                        pendingFlow: pendingFlows[status.provider],
                        isEnabled: enabledProviders.map { $0.contains(status.provider) } ?? true,
                        error: errors(status.provider),
                        showURLFallback: browserFallback.contains(status.provider),
                        onConnect: onConnect,
                        onDisconnect: onDisconnect,
                        onCancel: onCancel,
                        onToggle: onToggle,
                        onCopy: onCopy
                    )
                    .padding(.horizontal, Spacing.md)
                    .padding(.vertical, Spacing.sm)
                }

                if Footer.self != EmptyView.self {
                    Divider()
                        .foregroundStyle(palette.border)
                    footer()
                        .padding(Spacing.md)
                }
            }
            .background(palette.surface)
            .overlay(
                RoundedRectangle(cornerRadius: CornerRadius.lg)
                    .stroke(palette.border, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: CornerRadius.lg))
        }
    }
}
