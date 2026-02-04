import Foundation
import Observation

@MainActor
@Observable
public final class AuthState {
    private let authService: AuthService
    private var refreshTask: Task<Void, Never>?

    public private(set) var token: String?
    public private(set) var isLoading = false
    public private(set) var error: AuthError?

    public var isAuthenticated: Bool { token != nil && !isExpired }

    public var isExpired: Bool {
        guard let exp = authService.tokenExpiresAt() else { return true }
        return Date() >= exp
    }

    public var needsRefresh: Bool {
        guard let exp = authService.tokenExpiresAt() else { return false }
        return Date().addingTimeInterval(24 * 3600) >= exp
    }

    public init(authService: AuthService = AuthService()) {
        self.authService = authService
        self.token = authService.currentToken()
        startRefreshMonitor()
    }

    public func signIn() async {
        isLoading = true
        error = nil
        do {
            token = try await authService.signIn()
        } catch let e as AuthError {
            error = e
        } catch {
            self.error = .unknown(error)
        }
        isLoading = false
    }

    public func signOut() {
        try? authService.signOut()
        token = nil
        error = nil
    }

    private func startRefreshMonitor() {
        refreshTask?.cancel()
        refreshTask = Task { [weak self] in
            guard let self else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3600))
                if self.needsRefresh && !self.isExpired {
                    await self.refreshTokenSilently()
                }
            }
        }
    }

    private func refreshTokenSilently() async {
        do {
            token = try await authService.refreshToken()
        } catch {
            if self.token == nil {
                self.error = (error as? AuthError) ?? .unknown(error)
            }
        }
    }
}
